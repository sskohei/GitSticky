# SQLite security compatibility patch

Source: the published sqlx-sqlite 0.9.0 crate from crates.io (MIT/Apache-2.0).
Only the libsqlite3-sys dependency constraint in Cargo.toml and Cargo.toml.orig
is changed, from >=0.30.1, <0.38.0 to >=0.39.1, <0.40.0.

This allows the bundled SQLite 3.54.1 security fix (RUSTSEC-2026-0459), while
retaining the reviewed SQLx driver. No driver source is modified. The main
Cargo.toml explicitly pins libsqlite3-sys to the 0.39 series. Remove this vendor
patch when a released SQLx driver permits the patched SQLite release.

Verify with cargo test --locked --manifest-path src-tauri/Cargo.toml --lib.

Published crate SHA-256: `488e99c397a62007e4229aec669a179816339afc6d2620ca6fa420dbee2e982c`. Driver source and licenses were verified byte-for-byte against this archive. The upstream Cargo.lock is omitted; this application uses src-tauri/Cargo.lock.
