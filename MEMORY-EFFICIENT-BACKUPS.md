# Memory-efficient backup and restore

## Executive summary

Large databases must not require equally large amounts of RAM to back them up.
The CLI now processes database trailers, encrypted file contents and HTTP
transfers in bounded buffers. No new key, configuration migration or archive
conversion is required. Existing backups remain readable, and new backups use
the same format that previous clients understand.

This is a memory/performance change, not permission to change any application's
production database or rotate its field keys. A full application recovery test
remains a separate acceptance step.

## What changed

| Stage | Previous behavior | New behavior |
| --- | --- | --- |
| MySQL dump verification | Load the entire SQL file as UTF-8 | Inspect the final 4 KiB for the last nonblank completion line; supports binary SQL bodies |
| Encryption/decryption | Several complete archive buffers | 64 KiB input buffer plus a bounded output buffer, using OpenSSL's incremental GCM API |
| Archive-key verification | Load/decrypt the entire archive in RAM | Authenticate the stream into a discard sink |
| SHA-256 | Load the entire encrypted file | 64 KiB reads |
| Upload | Load the complete file into an HTTP body | Stream the file with its exact Content-Length |
| Download | Buffer the complete HTTP response | Stream into a private staging file, then publish atomically |

The HTTP client does not buffer arbitrary upload error bodies. Upload redirects
are rejected rather than silently succeeding without replaying a streamed body;
the service must provide the correct presigned object URL. Signed download/upload
URLs are omitted from transfer error messages.

Restoration checks a provided cloud checksum after a fresh download, before
decryption/extraction/database restore. Cached files retain their existing
checksum check. AEAD authentication is required regardless of whether the server
provided a checksum.

## Encryption compatibility and security

The format is unchanged: version bytes `01 00`, 12-byte random nonce, 16-byte
authentication tag, then ciphertext. The AES-256 key is still SHA-256 of the
exact configured encryption-key string. Do not hex-decode or otherwise alter it.

The implementation uses the maintained [OpenSSL EVP wrapper](https://docs.rs/openssl/latest/openssl/symm/struct.Crypter.html),
not a custom AES, counter-mode or authentication implementation. OpenSSL is
vendored and statically linked into released musl binaries; users need not
install a system OpenSSL library. RustCrypto's previous AES-GCM implementation
remains a **test-only** independent compatibility oracle.

Streaming GCM emits plaintext before its final authentication result is known.
That plaintext stays in an owner-only staging file. Only a successful final tag
check allows atomic publication at the destination path. Wrong keys, damaged
tags/nonces/ciphertext and truncation cannot overwrite an existing destination
or expose a partial file to the restore pipeline. Archive extraction and database
restoration happen only after this gate. The new tests check these properties.

AES-GCM's single-message plaintext limit is **64 GiB minus 32 bytes**, applied to
the compressed archive, not the uncompressed database. Oversize messages fail
before processing, and the limit is checked during streaming too. Larger archives
would need a separately designed, versioned segmented format; this patch does
not invent one. See [NIST SP 800-38D, section 5.2.1](https://nvlpubs.nist.gov/nistpubs/Legacy/SP/nistspecialpublication800-38d.pdf).

## Disk, resource and recovery boundaries

- SQL dumps and compressed/decrypted archives still occupy plaintext disk space.
  Backup/restore workspaces are randomly named mode-0700 temporary directories;
  staging files are mode 0600, independent of a permissive caller umask.
- Normal errors/cancellation clean up owned temporary paths. SIGKILL, a power
  failure or filesystem error can leave private files for operator inspection;
  deletion is not a secure wipe. Use protected storage and sufficient free space.
- Failed downloads preserve an existing cache entry. Restores still need an
  operator-validated destination; the CLI is not a general empty-target guard.
- Payload memory is bounded. OS page cache, database-client processes, metadata
  collections and the number of archived filesystem entries have separate costs.
- Updater binaries and configuration/API metadata are not database-sized payloads
  and are outside this streaming change. Scheduling and automatic-update behavior
  are unchanged. Published releases can be picked up by existing auto-updaters.
- Keys, historical backups and field-encryption rotation procedures are unchanged.
  This release alone does not prove that a particular production application or
  identity-server upgrade can be restored successfully.
- Cloud/provider/account upload-size limits are unchanged. The large loopback
  transfer test is not evidence of a successful upload of that size to the cloud.

## Tests and how to repeat them

Use a Rust edition-2024-capable toolchain, C compiler, Make and Perl. Vendored
OpenSSL is built from the version pinned in Cargo.lock. The dependency update
adds the needed packages without upgrading unrelated locked dependencies.

```bash
cargo test --locked
```

Regular tests cover byte/NULL-preserving SQLite backup/archive/encryption/HTTP
restore, unchanged destinations on integrity failures, old/new archive
compatibility at block and 64-KiB boundaries, owner-only outputs, incomplete HTTP
bodies, chunked downloads and exact upload lengths. The 24-GiB MySQL trailer test
uses a sparse file: it tests verifier behavior, not full-content processing.

The separate ignored stress test writes and processes **real non-sparse file
contents**. It encrypts, authenticates and hashes the entire file, uploads and
downloads it over a bounded-buffer loopback server, verifies both transfer
checksums, decrypts, and compares every original/recovered plaintext byte.
It never calls CalmBackup's cloud service or any database server.

Compile first outside the memory ceiling, then run the compiled test binary in
a container. Example for a binary built on Ubuntu 24.04 (requires `jq` and Docker):

```bash
TEST_BINARY=$(cargo test --locked --no-run --message-format=json | jq -r 'select(.reason == "compiler-artifact" and .profile.test == true and .target.name == "calmbackup") | .executable')
test -x "$TEST_BINARY"
SCRATCH=$(mktemp -d)
docker run --rm --network=none --memory=256m --memory-swap=256m \
  -v "$TEST_BINARY:/test:ro" -v "$SCRATCH:/scratch" \
  -e CB_MEMORY_TEST_GIB=1 -e CB_MEMORY_TEST_DIR=/scratch \
  ubuntu:24.04 /test core::memory_tests::large_file_pipeline_under_memory_limit \
  --exact --ignored --nocapture
```

Use the same or a compatible C-library runtime as the build environment. Only
loopback networking is needed; the test passes with Docker `--network=none`.
`CB_MEMORY_TEST_GIB` accepts 1–32 (default 2). Allow roughly four times that amount
of free disk space. The test removes only its exclusively created temporary
subdirectory on normal completion. `CB_MYSQL_DUMP` optionally names a read-only
mounted existing SQL file for trailer verification; no rows are printed/changed.

The master-triggered release workflow runs the 1-GiB case under a 256-MiB ceiling
before version bump, builds and publication. Normal unit tests also gate releases.
The dedicated tag-release workflow's previously inactive test condition is fixed.

## Release procedure

### Local validation on 14 September 2026

- Original baseline: 59 tests passed. Updated suite: 72 regular tests passed.
- The separate 16-GiB stress test passed with **256 MiB RAM and no swap** allowed:
  17,179,869,221 plaintext bytes and 17,179,869,251 encrypted bytes; upload and
  download checksums matched, full authentication passed, and every restored byte
  matched. Reported process high-water RSS was **13,748 KiB (13.4 MiB)**. This is
  process RSS, not the container's filesystem page-cache accounting.
- The supplied 24.4-GB MySQL master's trailer also passed a read-only check.
  No real database rows were uploaded, imported, modified or printed for this test.
- The debug stress harness took 916.35 seconds including file generation and
  several complete passes; this is not an optimized production-backup benchmark.
  Its generated scratch files were removed automatically after success.
- Static Linux amd64 musl release build passed and the executable ran without
  system OpenSSL. Clippy completed with existing style/dead-code warnings; modified
  Rust files passed formatting checks, and both workflow YAML files parsed.

### Publishing

Only `calmbackup/cb-cli` is in this release scope. After local tests, review the
diff and push the CLI change to its existing `master` workflow. It runs tests,
increments the patch version, tags, builds Linux amd64/arm64 musl artifacts and
publishes their SHA256SUMS. Do not manually create a competing version bump/tag.
Verify the workflow result and actual downloaded release artifacts before
reporting publication as complete. No installer/cron or application deployment
is part of validating the new CLI locally.
