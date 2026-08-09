# Dependency security audit

Last reviewed: 2026-08-09  
Next review: 2026-11-09, or immediately after a `sqlx`/`teloxide` upgrade.

## Rust

`cargo audit` scans 318 locked crates and reports one medium advisory:
`RUSTSEC-2023-0071` for `rsa 0.9.10`. There is currently no fixed release.
The crate is locked only through optional `sqlx-mysql`; this workspace disables
SQLx default features and enables PostgreSQL only. The current production graph
contains no path to `rsa`:

```powershell
cargo tree -i rsa --edges normal,build,dev --locked
# warning: nothing to print
```

The remaining dependency graph passes:

```powershell
cargo audit --ignore RUSTSEC-2023-0071
```

Do not enable the SQLx MySQL feature until the advisory has a fixed upgrade or
a replacement has been reviewed.

`RUSTSEC-2026-0173` marks `proc-macro-error2 2.0.1` unmaintained. It is a
compile-time-only transitive dependency (`teloxide -> aquamarine ->
proc-macro-error2`) and is not linked as runtime moderation logic. Track it with
Teloxide upgrades.

## Frontend

`npm audit --omit=dev` reports zero production vulnerabilities.
