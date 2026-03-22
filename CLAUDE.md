# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Project Overview

`cb-cli` is a standalone Go CLI tool for zero-knowledge encrypted database backups. It dumps databases (MySQL, PostgreSQL, SQLite), compresses with tar.gz, encrypts with AES-256-GCM, and uploads to the CalmBackup API via presigned S3/R2 URLs.

## Commands

```bash
make build        # Build binary to bin/calmbackup
make test         # Run all tests
make test-int     # Run integration tests (requires database tools)
make lint         # Run golangci-lint
make ci           # Lint + test + build
```

## Architecture

- **`cmd/`** — Cobra CLI commands (run, restore, list, status, init, version)
- **`internal/config/`** — YAML config parsing and validation
- **`internal/crypto/`** — AES-256-GCM encryption, byte-compatible with PHP Encryptor
- **`internal/archive/`** — tar.gz archive creation and extraction
- **`internal/dumper/`** — Database dumpers (MySQL, PostgreSQL, SQLite) using CLI tools
- **`internal/api/`** — HTTP client for CalmBackup API
- **`internal/upload/`** — Presigned URL upload/download
- **`internal/prune/`** — Local backup retention pruning
- **`internal/backup/`** — Backup and restore orchestration

## Key Conventions

- Tests first: write tests before implementation
- All encryption must be byte-compatible with the PHP `cb-package` Encryptor
- Minimal dependencies: only cobra + yaml.v3, everything else is stdlib
- `os/exec` for database CLI tools (mysqldump, pg_dump, sqlite3) — no shell interpolation
