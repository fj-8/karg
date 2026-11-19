# karg

**Command line arguments that explain themselves.**

<p>
  <a href="https://github.com/fj-8/karg/actions/workflows/ci.yml"><img src="https://github.com/fj-8/karg/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue" alt="License: MIT OR Apache-2.0"></a>
  <img src="https://img.shields.io/badge/dependencies-0-informational" alt="Zero dependencies">
  <img src="https://img.shields.io/badge/unsafe-forbidden-success" alt="unsafe forbidden">
  <img src="https://img.shields.io/badge/rust-1.74%2B-orange" alt="MSRV 1.74">
  <img src="https://img.shields.io/badge/tests-48%20passing-brightgreen" alt="48 tests">
</p>

A `no_std`-style discipline applied to a CLI library: typed accessors, nested
subcommands, generated help, and errors that name the nearest valid flag — with
**no dependencies at all**, not even dev-dependencies. The edit distance, the
help wrapper and the integer coercion are all in-crate.

```console
$ crop --formt=webp a.png b.png
crop: error: unexpected argument '--formt', did you mean '--format'?
Try 'crop --help' for usage.
$ echo $?
2
```

That message costs you one `.opt("format", 'f', "Output container")` in the
source. Everything below is what `karg` generates from the declaration.

---

## Table of contents

- [Install](#install)
- [Quick start](#quick-start)
- [Syntax you get for free](#syntax-you-get-for-free)
- [Typed accessors](#typed-accessors)
- [Subcommands, nested](#subcommands-nested)
- [Generated help](#generated-help)
- [Error catalogue](#error-catalogue)
- [Design notes](#design-notes)
- [Measurements](#measurements)
- [Development](#development)
- [License](#license)

## Install

The crate is not on crates.io yet, so depend on the tag:

```toml
[dependencies]
karg = { git = "https://github.com/fj-8/karg", tag = "v0.3.0" }
```

Then `cargo build`. Nothing else is fetched — there is no `Cargo.lock` line to
review, no build script, no proc macro. MSRV is **1.74**.

## Quick start

```rust
use karg::Cmd;

fn main() {
    let cmd = Cmd::new("crop", "1.2.0")
        .about("Crop an image to a rectangle and write it out")
        .flag("verbose", 'v', "Print each step as it happens")
        .opt("format", 'f', "Output container").value_name("FMT").default("png")
        .opt("quality", 'q', "JPEG/WEBP quality, 1-100").value_name("N").done()
        .multi("tag", 't', "Metadata tag as KEY=VALUE, repeatable").value_name("KEY=VALUE").done()
        .arg("source", "Image to read")
        .arg("target", "Image to write");

    let m = cmd.parse_or_exit();

    let quality = m.number_or::<u8>("quality", 90).unwrap();
    println!(
        "{} -> {} as {quality}-{}",
        m.positional(0).unwrap_or("-"),
        m.positional(1).unwrap_or("-"),
        m.get_or("format", "png"),
    );
    for tag in m.strings("tag") {
        println!("  tag {tag}");
    }
}
```

`parse_or_exit` prints help/version to stdout, errors to stderr prefixed with
the command name, and exits `0` or `2` — the GNU convention. Call
[`Cmd::parse_std`] if you want the `Result` and prefer to render it yourself.

## Syntax you get for free

| Input | Meaning |
| --- | --- |
| `--verbose` | long flag |
| `--format=webp`, `--format webp` | long option, attached or detached |
| `-v`, `-vv`, `-vvv` | short flag, repeated occurrences counted |
| `-j8`, `-j 8`, `-j=8` | short option, three spellings |
| `-abj 8` | clustered shorts, last one takes the value |
| `--no-verbose` | explicit negation, still counts as "the user decided" |
| `-` | a positional, so `-` keeps meaning stdin |
| `--` | freeze: every later token is a positional, `-rf` included |
| `--help` / `-h`, `--version` / `-V` | built in, unless you claim the letter |

Sub-`--` positionals reach the subcommand, not the parent: `tool -v serve -p 9000`
sets `verbose` on `tool` and `port` on `serve`.

## Typed accessors

```rust
let m = cmd.parse(["-vv", "--format=webp", "-q", "82", "a.png", "b.png"]).unwrap();

m.count("verbose");                  // 2   — occurrence count
m.flag("verbose");                   // true
m.get("format");                     // Some("webp")
m.get_or("color", "auto");           // "auto" when undeclared value
m.strings("tag");                    // &["KEY=VALUE", ...] for `multi`
m.number::<u8>("quality");           // Ok(Some(82)) — coercion, typed
m.number_or::<u16>("jobs", 8);       // Ok(8)
m.positional(0);                     // Some("a.png")
m.is_set("format");                  // true — typed by the user
m.subcommand();                      // Some(("serve", &Matches))
```

`is_set` is the reason defaults are worth having: a default shows up in `get()`
but leaves `is_set()` false, so "the user asked for it" stays answerable.

## Subcommands, nested

```rust
use karg::Cmd;

let cmd = Cmd::new("svc", env!("CARGO_PKG_VERSION"))
    .flag("verbose", 'v', "Log every request")
    .subcommand(
        Cmd::new("remote", "1.0.0")
            .subcommand(
                Cmd::new("add", "1.0.0")
                    .flag("fetch", 'f', "Fetch after registering")
                    .arg("name", "Remote name")
                    .arg("url", "Repository URL"),
            )
            .require_subcommand(),
    )
    .require_subcommand();

let m = cmd.parse(["-v", "remote", "add", "-f", "origin", "https://example.com"]).unwrap();
assert!(m.flag("verbose"));
let ("remote", remote) = m.subcommand().unwrap() else { panic!() };
let ("add", add) = remote.subcommand().unwrap() else { panic!() };
assert_eq!(add.positional(0), Some("origin"));
```

Each level keeps its own match set, and the help path follows the invocation:
`svc remote add --help` prints `Usage: svc remote add [OPTIONS] <NAME> <URL>`.

## Generated help

This is verbatim output from `cargo run --example crop -- --help`:

```text
Crop an image to a rectangle and write it out

Usage: crop [OPTIONS] <SOURCE> <TARGET> [RECT]

Options:
  -v, --verbose          Print each step as it happens
  --lossless             Never re-encode lossy pixels
  -f, --format <FMT>     Output container [default: png]
  -q, --quality <N>      JPEG/WEBP quality, 1-100
  -t, --tag <KEY=VALUE>  Metadata tag as KEY=VALUE, repeatable [repeatable]
  -h, --help             Print this help
  -V, --version          Print version

Examples:
  crop a.png out.jpg -f jpeg -q 82 topleft
  crop -vv --format=webp a.png out.webp center
```

The option column aligns to the longest invocation, descriptions wrap at 100
characters, `[required]`, `[default: …]` and `[repeatable]` are appended by the
renderer, and `hidden()` keeps an option parseable but off the listing. A test
asserts no help line exceeds 100 columns, so the block above cannot regress into
a wall of text.

## Error catalogue

Every failure carries the tokens needed to fix it, and `kind()` gives a stable
ASCII slug for logs and tests:

| Variant | `kind()` | Rendered message |
| --- | --- | --- |
| `UnknownOption` | `unknown-option` | `unexpected argument '--colour', did you mean '--color'?` |
| `MissingValue` | `missing-value` | `option '--output' requires a value` |
| `UnexpectedValue` | `unexpected-value` | `'--verbose' is a flag and takes no value, but 'webp' was supplied (…)` |
| `InvalidNegation` | `invalid-negation` | `'--no-output' is only valid for boolean flags, but '--output' takes a value` |
| `MissingRequired` | `missing-required` | `the required argument --output was not provided` |
| `TooManyPositionals` | `too-many-positionals` | `unexpected positional argument #0 ('a.txt'); this command takes fewer arguments` |
| `InvalidInteger` | `invalid-integer` | `expected 'jobs' to be a valid u8, got 'many'` |
| `InvalidUtf8` | `invalid-utf8` | `argument '…' is not valid UTF-8; this tool requires UTF-8 input` |
| `SubcommandRequired` | `subcommand-required` | `a subcommand is required, expected one of: serve or stop` |
| `Help` / `Version` | `help` / `version` | the rendered text, exit code `0` |

Suggestions use an in-crate Levenshtein pass (two-row dynamic programming, cut
off at distance 2, at most three candidates). `--colour` offers `--color`;
`--xyz` offers nothing rather than guessing.

## Design notes

- **Zero dependencies.** `grep -c '^\[\[package\]\]' Cargo.lock` → 1 (the entry
  for `karg` itself). A karg CLI has no supply chain to audit.
- **`#![forbid(unsafe_code)]`.** Not `deny`: there is no configuration under
  which the exception is acceptable.
- **Consuming builder.** `Cmd` is immutable once declared and `Send + Sync`, so
  one static command description can serve every thread — a test parses from
  four threads against a single `OnceLock<Cmd>`.
- **No macro.** Derive-style APIs buy keystrokes and cost you the compile error
  pointing at your code. A `Cmd` is data: print it, store it, build it in a loop.
- **What is deliberately missing** — `OsStr`/lossy mode, env-var fallback, shell
  completion, derive macros, unicode-width alignment. Each needs a dependency or
  a code generator; the surface stays small enough to read in one sitting.

## Measurements

`cargo run --release --example bench --features bench` on a Windows 11 box with an Intel Xeon
E5-2666 v3 @ 2.90 GHz, Rust 1.100, median of three runs:

| scenario | ns / parse | parses / s |
| --- | ---: | ---: |
| 4 flag tokens | 2 512 | 398 125 |
| 13 mixed tokens (options, multi, `--`) | 4 775 | 209 433 |
| nested subcommand | 3 221 | 310 444 |
| `--help` render | 12 404 | 80 619 |
| typo + Levenshtein suggestions | 4 519 | 221 276 |
| declaring a 4-option command | 1 025 | — |

Read this as "parsing is never your bottleneck": `argv` is materialised into
owned `String`s and every value is stored, so the numbers include the allocation
pass, and a cold process start costs two to three orders of magnitude more than
the whole parse. The `--help` row is the widest spread (6.2–12.8 µs across
runs); it renders and wraps text, which is why it is listed at all.

No comparison against other parsers is published here, because a fair one needs
their code in the dependency tree — and this crate's premise is that it does not
need anyone else's.

## Development

```console
$ cargo fmt --check
$ cargo clippy --all-targets --all-features -- -D warnings
$ cargo test            # 35 unit + 10 integration + 3 doc tests
$ cargo run --release --example bench --features bench
```

- `src/error.rs` — the `Error` enum, exit codes, message rendering.
- `src/help.rs` — usage line, aligned option table, wrapping.
- `src/parse.rs` — the tokenizer and validation, plus the edit-distance suggester.
- `tests/api.rs` — black-box tests; only re-exported items are reachable, so a
  public-surface break fails even when every unit test still passes.
- `examples/` — `crop` (single command), `gitlike` (three levels), `bench`.

`.github/workflows/ci.yml` runs seven jobs, four of which are the same suite on a
different platform or toolchain: `Test ubuntu-latest / stable`,
`Test windows-latest / stable`, `Test macos-latest / stable` and
`Test ubuntu-latest / 1.74`. Each of those legs runs `cargo test --all-targets`,
`cargo test --doc` and the three example commands above as separate steps, so a
broken example names itself instead of failing inside "tests". The other three
jobs are `cargo fmt --all --check` with `clippy -D warnings`; a lock-file grep
that fails if a package other than `karg` appears; and `cargo doc` under
`RUSTDOCFLAGS=-D warnings`.
All seven are green on `main` when every leg gets a runner, and the five commits
from `0886512` to `d33975f` whose runs finished did not: `0886512`, `6c89e50`,
`6a1c077`, `f46b112` and `d33975f` each reported `failure` with six jobs green and
`Test macos-latest / stable` `cancelled` with no runner ever assigned to it — no
steps, no start time, nothing to read. Two of the five were re-run on the same
commit and the leg was cancelled again, `6a1c077` after twenty-five minutes in the
queue with its `started_at` still empty. The two commits before that run, `a7f9d9b`
and `fe809ba`, had the identical leg scheduled on the first attempt and all seven
jobs green, which is the state the badge is meant to describe. Both ends of the
range are closed on purpose: what the next five finished runs say is a different
sentence, and `gh api repos/fj-8/karg/actions/runs?branch=main&event=push`, read
against the `jobs_url` of each, is how this one was written. Recorded here because
a badge cannot tell a real failure from a starved queue, and a run with one
cancelled job in it is reported as a `failure` rather than as something amber.

## Elsewhere in this account

The same rules applied in three other languages — zero dependencies, and every
number in the README measured in the same checkout:

- [`bitlane`](https://github.com/fj-8/bitlane) — header-only C++20 compressed
  bitmap index over `uint32_t` sets, with a versioned, FNV-1a-64-checked wire
  format.
- [`sift`](https://github.com/fj-8/sift) — pure-stdlib Python 3.11+ streaming
  estimators (HyperLogLog, Count-Min, t-digest, Welford, reservoir) and a CLI;
  its benchmark transcript is committed and a test executes its own README.
- [`turnstile`](https://github.com/fj-8/turnstile) — bounded MPMC queue over a
  fixed ring, the turn-based protocol on `VarHandle` with no locks and no
  allocation per element; its transcripts are committed too, including the
  measurement that argued against a padding trick the design note liked.

## License

Dual-licensed under either of [Apache-2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option.

### Contribution

Unless you state otherwise, any contribution you submit for inclusion in the
work shall be dual-licensed as above, without any additional terms or
conditions.
