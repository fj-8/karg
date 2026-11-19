# Changelog

All notable changes to `karg` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- A root `LICENSE` file stating both grants, the `MIT OR Apache-2.0` SPDX
  identifier that heads every source file, and the contribution clause. The full
  texts were already in `LICENSE-APACHE` and `LICENSE-MIT`; this is the filename
  tools reach for when they do not read the README. No terms changed.

### Changed
- The README's CI paragraph names the seven jobs as the Actions tab lists them:
  four legs of one matrix (`ubuntu`/stable, `windows`/stable, `macos`/stable,
  `ubuntu`/1.74), each of which runs `cargo test --all-targets`, `cargo test
  --doc` and the three example commands as separate steps, plus Format and lints,
  Zero dependencies and Documentation. The old sentence read as if the five
  commands were five jobs, which is not what anyone comparing it to a run list
  would find.

## [0.3.0] - 2026-11-18

Initial public release.

### Added
- `Cmd` builder: `flag`, `long_flag`, `opt`, `long_opt`, `multi`, `arg`,
  `opt_arg`, `rest`, `subcommand`, `require_subcommand`, `about`, `after_help`.
- `ArgBuilder` modifiers `default`, `required`, `value_name`, `hidden`, which
  forward every declaration method so a chain never needs an explicit `done()`.
- Parser for `--long`, `--long=value`, `-s value`, `-svalue`, `-s=value`,
  clustered shorts (`-abj 8`), `--no-flag` negation, `--` passthrough, a bare
  `-` as a positional, and built-in `-h`/`-V` fallbacks.
- Nested subcommands with per-level match sets and an invocation-aware usage
  line (`Usage: svc remote add [OPTIONS] <NAME> <URL>`).
- `Matches` accessors `command`, `is_set`, `flag`, `count`, `get`, `get_or`,
  `strings`, `number`, `number_or`, `positional`, `positionals`, `subcommand`.
- `Error` with nine failure variants plus `Help`/`Version` display requests,
  stable `kind()` slugs, GNU `exit_code()` mapping (0 vs 2) and Levenshtein
  suggestions capped at distance 2.
- Help renderer: aligned option table, `[required]` / `[default: …]` /
  `[repeatable]` trailers, wrapping at 100 columns, hidden-option omission.
- `examples/crop.rs`, `examples/gitlike.rs`, `examples/bench.rs`.
- 35 unit tests, 10 black-box integration tests, 3 doc tests.

### Notes
- Zero dependencies, zero dev-dependencies, `#![forbid(unsafe_code)]`, MSRV 1.74.
- Published numbers in `README.md` are measured, not estimated; see
  `cargo run --release --example bench`.

[Unreleased]: https://github.com/fj-8/karg/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/fj-8/karg/releases/tag/v0.3.0
