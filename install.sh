#!/usr/bin/env bash
#
# CalmBackup CLI Installer
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/calmbackup/cb-cli/master/install.sh | bash
#
# As root (recommended for production servers):
#   - Binary:  /usr/local/bin/calmbackup
#   - Config:  /etc/calmbackup/calmbackup.yaml
#   - Backups: /var/backups/calmbackup/
#   - Cron:    /etc/cron.d/calmbackup (daily at 2:00 AM)
#
# As regular user:
#   - Binary:  ~/.local/bin/calmbackup
#   - Config:  ~/.config/calmbackup/calmbackup.yaml
#   - Backups: ~/.local/share/calmbackup/
#   - Cron:    user crontab (daily at 2:00 AM)
#

set -euo pipefail

REPO="calmbackup/cb-cli"
BINARY="calmbackup"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${GREEN}[+]${NC} $*"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
error() { echo -e "${RED}[x]${NC} $*" >&2; }
bold()  { echo -e "${BOLD}$*${NC}"; }

# --- Detect privilege level ---

if [ "$(id -u)" -eq 0 ]; then
    MODE="system"
    INSTALL_DIR="/usr/local/bin"
    CONFIG_DIR="/etc/calmbackup"
    BACKUP_DIR="/var/backups/calmbackup"
else
    MODE="user"
    INSTALL_DIR="${HOME}/.local/bin"
    CONFIG_DIR="${HOME}/.config/calmbackup"
    BACKUP_DIR="${HOME}/.local/share/calmbackup"
fi

CONFIG_FILE="${CONFIG_DIR}/calmbackup.yaml"

info "Install mode: ${MODE}"

# --- Pre-flight checks ---

if ! command -v curl &>/dev/null; then
    error "curl is required but not installed."
    exit 1
fi

# --- Detect architecture ---

ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64)  ARCH="amd64" ;;
    aarch64) ARCH="arm64" ;;
    arm64)   ARCH="arm64" ;;
    *)
        error "Unsupported architecture: ${ARCH}"
        echo "  Supported: x86_64 (amd64), aarch64/arm64"
        exit 1
        ;;
esac

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
if [ "${OS}" != "linux" ]; then
    error "Unsupported OS: ${OS}. Only Linux is supported."
    exit 1
fi

info "Detected platform: ${OS}/${ARCH}"

# --- Download latest release ---

info "Fetching latest release..."
LATEST="$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p')"

if [ -z "${LATEST}" ]; then
    error "Could not determine latest release. Check https://github.com/${REPO}/releases"
    exit 1
fi

info "Latest version: ${LATEST}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

# Try both naming conventions (calmbackup_ for >=v1.0.1, cb-cli_ for v1.0.0)
DOWNLOADED=false
for PREFIX in calmbackup cb-cli; do
    TARBALL="${PREFIX}_${LATEST#v}_${OS}_${ARCH}.tar.gz"
    URL="https://github.com/${REPO}/releases/download/${LATEST}/${TARBALL}"
    info "Downloading ${TARBALL}..."
    if curl -fsSL "${URL}" -o "${TMP_DIR}/${TARBALL}" 2>/dev/null; then
        DOWNLOADED=true
        break
    fi
done

if [ "${DOWNLOADED}" = false ]; then
    error "Download failed. Check that a release exists for ${OS}/${ARCH}."
    echo "  https://github.com/${REPO}/releases"
    exit 1
fi

# --- Install binary ---

mkdir -p "${INSTALL_DIR}"
info "Installing to ${INSTALL_DIR}/${BINARY}..."
tar -xzf "${TMP_DIR}/${TARBALL}" -C "${TMP_DIR}"
install -m 755 "${TMP_DIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"

VERSION="$("${INSTALL_DIR}/${BINARY}" version 2>/dev/null || echo "${LATEST}")"
info "Installed: ${VERSION}"

# Check if INSTALL_DIR is in PATH
if ! echo "${PATH}" | tr ':' '\n' | grep -qx "${INSTALL_DIR}"; then
    warn "${INSTALL_DIR} is not in your PATH."
    echo "  Add it with: export PATH=\"${INSTALL_DIR}:\${PATH}\""
    echo "  Or add that line to your ~/.bashrc / ~/.zshrc"
fi

# --- Config directory ---

mkdir -p "${CONFIG_DIR}"
chmod 700 "${CONFIG_DIR}"

if [ ! -f "${CONFIG_FILE}" ]; then
    info "Creating config template at ${CONFIG_FILE}..."
    cat > "${CONFIG_FILE}" << YAML
# CalmBackup configuration
# Docs: https://github.com/calmbackup/cb-cli
#
# Get your API key from https://app.calmbackup.com/dashboard

api_key: ""
encryption_key: ""

# Database to back up
# Supported drivers: mysql, pgsql, sqlite
database:
  driver: mysql
  host: "127.0.0.1"
  port: 3306
  database: ""
  username: ""
  password: ""
  # path: ""  # sqlite only

# Additional directories to include in backup (optional)
directories: []

# Where to store local encrypted backups
local_path: "${BACKUP_DIR}"

# How many days to keep local backups
local_retention_days: 7
YAML
    chmod 600 "${CONFIG_FILE}"
    info "Config template created. Edit it with your credentials."
else
    warn "Config already exists at ${CONFIG_FILE}, skipping."
fi

# --- Backup directory ---

mkdir -p "${BACKUP_DIR}"
chmod 750 "${BACKUP_DIR}"
info "Backup directory: ${BACKUP_DIR}"

# --- Cron job ---

if [ "${MODE}" = "system" ]; then
    CRON_FILE="/etc/cron.d/calmbackup"
    if [ ! -f "${CRON_FILE}" ]; then
        info "Installing daily cron job..."
        cat > "${CRON_FILE}" << 'CRON'
# CalmBackup - daily encrypted database backup
# Edit time as needed. Default: 2:00 AM
# Logs go to syslog (view with: journalctl -t calmbackup)

SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin

0 2 * * * root calmbackup run --quiet 2>&1 | logger -t calmbackup
CRON
        chmod 644 "${CRON_FILE}"
        info "Cron installed: daily at 2:00 AM (edit ${CRON_FILE} to change)"
    else
        warn "Cron job already exists at ${CRON_FILE}, skipping."
    fi
else
    # User-level cron via crontab
    CRON_LINE="0 2 * * * ${INSTALL_DIR}/calmbackup run --quiet 2>&1 | logger -t calmbackup"
    if crontab -l 2>/dev/null | grep -qF "calmbackup run"; then
        warn "Cron job already exists in user crontab, skipping."
    else
        info "Installing daily cron job in user crontab..."
        (crontab -l 2>/dev/null || true; echo "# CalmBackup - daily encrypted database backup"; echo "${CRON_LINE}") | crontab -
        info "Cron installed: daily at 2:00 AM (edit with: crontab -e)"
    fi
fi

# --- Done ---

echo
echo -e "${BOLD}"
echo '  🪷 CalmBackup'
echo -e "${NC}"
bold "  Installed successfully — ${VERSION}"
echo
echo -e "  ${GREEN}✓${NC} AES-256-GCM encryption — your data is encrypted before"
echo "    it ever leaves your server. Zero-knowledge: we can't read it."
echo -e "  ${GREEN}✓${NC} Daily automated backups at 2:00 AM via cron"
echo -e "  ${GREEN}✓${NC} Local + cloud storage for redundancy"
echo
echo -e "  ${BOLD}Get started:${NC}"
echo
echo -e "  ${BOLD}1.${NC} Get your API key ${GREEN}(free)${NC}:"
echo -e "     ${GREEN}https://app.calmbackup.com/register${NC}"
echo
if [ "${MODE}" = "system" ]; then
    echo -e "  ${BOLD}2.${NC} Run the setup wizard:"
    echo "     sudo calmbackup init"
else
    echo -e "  ${BOLD}2.${NC} Run the setup wizard:"
    echo "     calmbackup init"
fi
echo
echo -e "  ${BOLD}3.${NC} Run your first backup:"
echo "     calmbackup run"
echo
echo "  That's it. Your backups are encrypted, automated, and safe."
echo
echo -e "  Docs: ${GREEN}https://calmbackup.com/docs${NC}"
if [ "${MODE}" = "system" ]; then
    echo "  Logs: journalctl -t calmbackup"
else
    echo "  Logs: journalctl -t calmbackup"
    echo "  Cron: crontab -e"
fi
echo
