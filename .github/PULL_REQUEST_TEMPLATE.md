<!--
Short is fine. A two-line fix with a failing test first is the ideal PR here.
-->

## What changes

## Why it is not expressible today

## Verification

- [ ] `cargo test` passes (including the new test)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [ ] `cargo fmt --all --check` is clean
- [ ] `cargo +1.74.0 test` still builds — MSRV is part of the contract
- [ ] `cargo tree` still prints exactly one package

## If this touches performance

Paste `cargo run --release --example bench --features bench` before and after, same machine, same
terminal. No numbers, no merge.
