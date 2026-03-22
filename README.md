# CalmBackup CLI

Zero-knowledge encrypted database backup CLI for Linux. Dumps your database, compresses it with any additional files, encrypts everything with AES-256-GCM, and uploads to CalmBackup's cloud storage. Your encryption key never leaves your server.

Supports **MySQL**, **PostgreSQL**, and **SQLite**.

## Install

```bash
curl -sSL https://raw.githubusercontent.com/calmbackup/cb-cli/master/install.sh | sudo bash
```

This will:
- Install the `calmbackup` binary to `/usr/local/bin/`
- Create a config template at `/etc/calmbackup/calmbackup.yaml`
- Create the backup directory at `/var/backups/calmbackup/`
- Set up a daily cron job (2:00 AM) at `/etc/cron.d/calmbackup`

## Setup

After install, either edit the config directly:

```bash
sudo nano /etc/calmbackup/calmbackup.yaml
```

Or run the interactive setup:

```bash
sudo calmbackup init
```

You'll need:
- Your **API key** from [app.calmbackup.com/dashboard](https://app.calmbackup.com/dashboard)
- Your **database credentials**

The setup generates an encryption key automatically and saves it to `/etc/calmbackup/calmbackup-recovery-key.txt`. **Store this key somewhere safe** — without it, your backups cannot be decrypted.

## Configuration

Config location (searched in order):
1. `--config <path>` flag (explicit)
2. `/etc/calmbackup/calmbackup.yaml` (system-wide)
3. `~/.config/calmbackup/calmbackup.yaml` (user-level)
4. `./calmbackup.yaml` (current directory)

```yaml
api_key: "ak_live_..."
encryption_key: "64-char-hex-key"

database:
  driver: mysql           # mysql | pgsql | sqlite
  host: "127.0.0.1"
  port: 3306
  database: "myapp"
  username: "root"
  password: "secret"
  # path: "/path/to.db"  # sqlite only

directories:              # additional files to include (optional)
  - /var/www/app/uploads

local_path: "/var/backups/calmbackup"
local_retention_days: 7
```

## Usage

```bash
calmbackup run              # Run a backup
calmbackup list             # List local and cloud backups
calmbackup status           # Check connectivity and local backup info
calmbackup restore <id>     # Restore a backup by ID
calmbackup version          # Print version
```

### Flags

```
--config <path>    Override config file location
--verbose, -v      Verbose output
--quiet, -q        Suppress non-error output (useful for cron)
```

## Cron

The installer sets up a daily cron job automatically. To customize the schedule:

```bash
sudo nano /etc/cron.d/calmbackup
```

Default schedule: daily at 2:00 AM. Logs go to syslog:

```bash
journalctl -t calmbackup
```

## How it works

1. **Dump** — Runs `mysqldump`, `pg_dump`, or `sqlite3 .backup` depending on your driver
2. **Archive** — Creates a `.tar.gz` with the dump and any configured directories
3. **Encrypt** — AES-256-GCM encryption with your key (zero-knowledge: the server never sees your key)
4. **Upload** — Uploads the encrypted file to CalmBackup via presigned S3 URL
5. **Confirm** — Verifies the upload with checksum validation
6. **Prune** — Cleans up local backups older than retention period

## Building from source

```bash
git clone https://github.com/calmbackup/cb-cli.git
cd cb-cli
make build       # Output: bin/calmbackup
make test        # Run all tests
```

Requires Go 1.24+.

## License

Proprietary. See LICENSE file.
