# CalmBackup CLI

Zero-knowledge encrypted database backup CLI for Linux. Dumps your database, compresses it with any additional files, encrypts everything with AES-256-GCM, and uploads to CalmBackup's cloud storage. Your encryption key never leaves your server.

Supports **MySQL**, **PostgreSQL**, and **SQLite**.

## Install

```bash
curl -sSL https://raw.githubusercontent.com/calmbackup/cb-cli/master/install.sh | bash
```

The installer auto-detects whether you're running as root or a regular user:

**As root** (`sudo bash`):
| | Path |
|---|---|
| Binary | `/usr/local/bin/calmbackup` |
| Config | `/etc/calmbackup/calmbackup.yaml` |
| Backups | `/var/backups/calmbackup/` |
| Cron | `/etc/cron.d/calmbackup` |

**As regular user** (`bash`):
| | Path |
|---|---|
| Binary | `~/.local/bin/calmbackup` |
| Config | `~/.config/calmbackup/calmbackup.yaml` |
| Backups | `~/.local/share/calmbackup/` |
| Cron | user crontab (`crontab -e`) |

Both modes set up a daily cron job at 2:00 AM automatically.

## Setup

After install, run the interactive setup:

```bash
calmbackup init        # as regular user
sudo calmbackup init   # as root
```

Or edit the config directly:

```bash
nano ~/.config/calmbackup/calmbackup.yaml   # user install
sudo nano /etc/calmbackup/calmbackup.yaml   # root install
```

You'll need:
- Your **API key** from [app.calmbackup.com/dashboard](https://app.calmbackup.com/dashboard)
- Your **database credentials**

The setup generates an encryption key automatically and saves a recovery key file. **Store this key somewhere safe** — without it, your backups cannot be decrypted.

## Configuration

Config location (searched in order):
1. `--config <path>` flag (explicit)
2. `/etc/calmbackup/calmbackup.yaml` (system-wide)
3. `~/.config/calmbackup/calmbackup.yaml` (user-level, XDG-compliant)
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

local_path: "/var/backups/calmbackup"  # or ~/.local/share/calmbackup for user installs
local_retention_days: 7
```

## Usage

### MySQL: one snapshot of multiple databases

Replace `database: "myapp"` with an explicit list in the `database` section:

```yaml
database:
  driver: mysql
  host: "127.0.0.1"
  port: 3306
  username: "backup_operator"
  password: "configure-privately"
  databases: [bb_api, bb_spine, keycloak]
```

Do not set both `database` and `databases`. The list is MySQL-only; existing
single-database configurations remain supported. One `mysqldump` invocation uses
`--single-transaction --databases`, including routines, triggers and events.
The result is one encrypted archive, not three independently timed snapshots.
Transactional consistency requires transactional tables (such as InnoDB) and
no concurrent schema changes. Coordinate deployment/migration locks separately;
this CLI option does not stop applications or establish those locks.

Restoring this format uses the dump's **original database names**, including its
`CREATE DATABASE` and `USE` statements. It does not rename, filter or sandbox the
SQL to the configured list. Verify the selected archive and restore first to an
isolated empty MySQL server with the intended privileges. Never point a rehearsal
at production. Configuration/files, archive keys and application field keys need
their own recovery copies. A synthetic two-server MySQL 8.0.45 round-trip passed
for three schemas, Unicode/binary/NULL values, schema defaults/column metadata,
cross-schema foreign keys, views, routines, triggers and events. A separate
synthetic concurrent-write test committed transactions across all three schemas
while dumping 40 MiB of payload: every restored schema had the same transaction
revision, with all payload rows present. Deployment locking remains the caller's
responsibility.

### Commands

```bash
calmbackup run              # Run a backup
calmbackup list             # List local and cloud backups
calmbackup status           # Check connectivity and local backup info
calmbackup restore <id>     # Restore a backup by ID
calmbackup version          # Print version
```

### Retry a retained archive (unreleased)

The staged `upload` command retries one encrypted archive without connecting to
the database, exporting data again, pruning backups, or automatically retrying
HTTP requests. It is not available in released v2.0.12 yet.

```bash
calmbackup --config /private/calmbackup.json --json upload \
  /private/backup-20260917-010000.tar.gz.enc --sha256 <recorded-sha256>
```

Use the configuration for the intended cloud account and the archive's original
encryption key. The checksum must be a previously checked lowercase SHA-256, not
a value accepted blindly from an untrusted file. The command creates an
owner-only **encrypted** temporary snapshot (set `TMPDIR` to private disk-backed
storage and allow free space equal to the archive size), checks its checksum and authenticates the entire archive
before making cloud requests. It never writes decrypted data.

All confirmed-backup pages are checked. An exact existing match is returned
without reuploading; duplicate identities, repeated pages or conflicting metadata
fail closed. A new upload is confirmed and checked again through the metadata API.
The JSON receipt deliberately reports `restoration_verified: false`: download,
decryption and an isolated restore are still required. Pending uploads are not
visible in the confirmed-backup API; a failed attempt may leave a pending cloud
record. Do not manually confirm that record or mark an earlier failed run successful.
The original local archive is retained on both success and failure. This command
does not self-update, and it does not fix an underlying transport problem.

### Flags

```
--config <path>    Override config file location
--json             Output JSON in CLI mode
--quiet, -q        Suppress non-error output (useful for cron)
--no-auto-update   Skip pre-backup self-update (operator-managed upgrades)
```

## Cron

The installer sets up a daily cron job automatically. To customize the schedule:

```bash
# Root install
sudo nano /etc/cron.d/calmbackup

# User install
crontab -e
```

Default schedule: daily at 2:00 AM. Logs go to syslog:

```bash
journalctl -t calmbackup
```

## Automatic updates

Every `calmbackup run` checks for a newer stable release before opening the
database. When one is available, the CLI verifies the release archive against
the published SHA-256 manifest, installs it atomically under a global update
lock, and restarts the pending backup with the new binary. The previous binary
is retained beside the installation as `calmbackup.previous` for rollback.

An unavailable update service or an installation-permission error does not
prevent the scheduled backup from running. Update checks time out quickly, and
installation errors are written to stderr/syslog.

The `--no-auto-update` option skips both the update check and binary
replacement before `run`, for deployments where an operator installs and verifies
specific releases. Example: `calmbackup --no-auto-update run --config /private/config.yaml`.
Automatic updates remain enabled by default. This option does not verify the
installed binary for you and does not disable interactive dashboard update actions;
operators opting out must arrange reviewed upgrades and security fixes themselves.
Do not add this flag to an older installed CLI that does not advertise it in help.

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

Requires Rust with edition 2024 support, a C compiler, Make and Perl. SQLite and
OpenSSL are built from vendored sources; released Linux musl binaries do not need
system OpenSSL. CI builds using the committed Cargo.lock.

### Multi-database release checks

Both release workflows now run the two opt-in MySQL integration tests against
fresh synthetic MySQL 8.0.45 server pairs. They test schema/value/object recovery
and snapshot consistency while cross-schema transactions commit concurrently.
The launcher refuses non-CI hosts; never point these tests at application data.

The launcher's failure/cleanup logic can be tested locally without Docker or
network access; these tests replace all external commands with synthetic stubs:

```sh
python3 .github/scripts/test_mysql_launcher.py
```

These launcher tests are not database integration tests. The existing ignored
MySQL tests independently require empty servers, distinct verified UUIDs and
disabled event scheduling before creating any fixture data.

## Large backups and memory

Database dump verification, AES-256-GCM encryption/decryption, SHA-256 checksums
and HTTP transfers use bounded buffers instead of reading whole backups into RAM.
The existing archive format and encryption keys remain compatible. Restore waits
for full authentication before extracting any plaintext or touching a database.

Temporary plaintext still needs private **disk space**; streaming does not mean
there is no plaintext on disk. See [memory limits, compatibility and the repeatable
acceptance test](MEMORY-EFFICIENT-BACKUPS.md).

## License

Proprietary. See LICENSE file.
