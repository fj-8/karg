# Contributing to karg

Thanks for taking the look. This is a small library with three unusual
constraints, so please read them before writing code.

## The three rules

1. **No dependencies.** Not in `[dependencies]`, not in `[dev-dependencies]`.
   A patch that adds one will be closed with a discussion, not merged. The point
   of the crate is that `cargo tree` has exactly one line.
2. **No `unsafe`.** `#![forbid(unsafe_code)]` is compile-enforced; an
   `#[allow(unsafe_code)]` will not be accepted either.
3. **No fabricated numbers.** If a change touches performance, paste the output
   of `cargo run --release --example bench --features bench` for before and after on the same
   machine. Do not quote other parsers' benchmarks.

## Getting set up

```console
$ git clone https://github.com/fj-8/karg && cd karg
$ cargo test                        # 35 unit + 10 integration + 3 doc tests
$ cargo clippy --all-targets --all-features -- -D warnings
$ cargo fmt --check
```

Rust 1.74 or newer. Nothing else to install — no `cargo-hack`, no `nextest`, no
`miri`, because those would be dependencies of a different kind.

## Where the code lives

| File | Owns |
| --- | --- |
| `src/error.rs` | `Error` variants, `kind()` slugs, message text, exit codes |
| `src/parse.rs` | tokenizer, validation, subcommand descent, edit distance |
| `src/help.rs` | usage line, option table alignment, wrapping |
| `src/lib.rs` | the public types and the builder chains |

New syntax goes in `parse.rs` with a unit test there *and* a black-box test in
`tests/api.rs`. The integration file only sees re-exported items, which is what
makes it the guard on the public surface.

## Adding a feature

Open an issue first if it changes the API. The builder is deliberately
overloaded already — `flag` vs `long_flag`, `opt` vs `long_opt`, `arg` vs
`opt_arg` vs `rest` — and a seventh way to declare something is worse than an
awkward combination of the six.

Things that will not be added: env-var fallback, shell completion, derive
macros, `OsStr` mode, unicode-width alignment. Each needs a dependency or a
generator, and each is listed under "deliberately not here" in the crate docs.

## Commit style

Imperative subject, 72 columns, and the body explains why rather than what.
One logical change per commit. Tests ship in the same commit as the code they
cover.

## License

By contributing you agree that your contributions are licensed under the
project's MIT OR Apache-2.0 dual license, without additional terms.
