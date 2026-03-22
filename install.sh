#!/usr/bin/env bash
#
# CalmBackup CLI Installer
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/calmbackup/cb-cli/master/install.sh | sudo bash
#
# What it does:
#   1. Downloads the latest calmbackup binary to /usr/local/bin/
#   2. Creates /etc/calmbackup/ with a config template (if not already present)
#   3. Creates /var/backups/calmbackup/ for local backup storage
#   4. Installs a daily cron job at /etc/cron.d/calmbackup
#
# After install, edit /etc/calmbackup/calmbackup.yaml with your credentials,
# or run: sudo calmbackup init
#

set -euo pipefail

REPO="calmbackup/cb-cli"
BINARY="calmbackup"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/calmbackup"
CONFIG_FILE="${CONFIG_DIR}/calmbackup.yaml"
BACKUP_DIR="/var/backups/calmbackup"
CRON_FILE="/etc/cron.d/calmbackup"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${GREEN}[+]${NC} $*"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
error() { echo -e "${RED}[x]${NC} $*" >&2; }
bold()  { echo -e "${BOLD}$*${NC}"; }

# --- Pre-flight checks ---

if [ "$(id -u)" -ne 0 ]; then
    error "This installer must be run as root."
    echo "  Usage: curl -sSL https://raw.githubusercontent.com/calmbackup/cb-cli/master/install.sh | sudo bash"
    exit 1
fi

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

TARBALL="calmbackup_${LATEST#v}_${OS}_${ARCH}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${LATEST}/${TARBALL}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

info "Downloading ${TARBALL}..."
if ! curl -sSL "${URL}" -o "${TMP_DIR}/${TARBALL}"; then
    error "Download failed. Check that a release exists for ${OS}/${ARCH}."
    echo "  URL: ${URL}"
    exit 1
fi

# --- Install binary ---

info "Installing to ${INSTALL_DIR}/${BINARY}..."
tar -xzf "${TMP_DIR}/${TARBALL}" -C "${TMP_DIR}"
install -m 755 "${TMP_DIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"

VERSION="$("${INSTALL_DIR}/${BINARY}" version 2>/dev/null || echo "${LATEST}")"
info "Installed: ${VERSION}"

# --- Config directory ---

mkdir -p "${CONFIG_DIR}"
chmod 700 "${CONFIG_DIR}"

if [ ! -f "${CONFIG_FILE}" ]; then
    info "Creating config template at ${CONFIG_FILE}..."
    cat > "${CONFIG_FILE}" << 'YAML'
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
local_path: "/var/backups/calmbackup"

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

# --- Done ---

echo
bold "CalmBackup CLI installed successfully!"
echo
echo "Next steps:"
echo
echo "  1. Edit the config with your credentials:"
echo "     sudo nano ${CONFIG_FILE}"
echo
echo "  2. Or run the interactive setup:"
echo "     sudo calmbackup init"
echo
echo "  3. Test connectivity:"
echo "     calmbackup status"
echo
echo "  4. Run your first backup:"
echo "     calmbackup run"
echo
echo "  Backups will run automatically every day at 2:00 AM."
echo "  Logs: journalctl -t calmbackup"
echo
