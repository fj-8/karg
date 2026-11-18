//! Throughput probe for the parse path.
//!
//! ```console
//! $ cargo run --release --example bench
//! ```
//!
//! Numbers are measured on the machine you are sitting at, over a window of at
//! least 250 ms per row. `karg` is timed against nothing but itself; the last row
//! is the builder chain rerun per iteration, i.e. the price of declaring a
//! command fresh instead of reusing one.

use karg::Cmd;
use std::hint::black_box;
use std::time::{Duration, Instant};

/// Smallest sample window; long enough that timer resolution is noise.
/// Built per call because `Duration::from_millis` is not a `const fn` on the MSRV.
const MIN_WINDOW_MS: u64 = 250;

fn min_window() -> Duration {
    Duration::from_millis(MIN_WINDOW_MS)
}

fn main() {
    let flags = Cmd::new("bench", "1.0.0").flag("verbose", 'v', "").flag("quiet", 'q', "");

    let mixed = Cmd::new("bench", "1.0.0")
        .flag("verbose", 'v', "")
        .opt("output", 'o', "")
        .done()
        .opt("jobs", 'j', "")
        .default("8")
        .done()
        .multi("tag", 't', "")
        .done()
        .arg("src", "")
        .arg("dst", "")
        .rest("extra", "");

    let nested = Cmd::new("bench", "1.0.0")
        .subcommand(
            Cmd::new("remote", "1.0.0")
                .subcommand(
                    Cmd::new("add", "1.0.0").flag("fetch", 'f', "").arg("name", "").arg("url", ""),
                )
                .require_subcommand(),
        )
        .require_subcommand();

    println!(
        "karg {} · {} · {} · >= {MIN_WINDOW_MS} ms per row",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    println!();
    println!("{:<24} {:>10} {:>14}", "scenario", "ns/parse", "parses/s");

    row("4 flag tokens", &flags, &["-v", "-vv", "--verbose", "--no-quiet"]);
    row(
        "13 mixed tokens",
        &mixed,
        &[
            "-v",
            "-o",
            "out.tar",
            "--jobs=16",
            "-t",
            "a=1",
            "-t",
            "b=2",
            "src",
            "dst",
            "--",
            "rm",
            "-rf",
        ],
    );
    row("nested subcommand", &nested, &["remote", "add", "-f", "origin", "https://example.com"]);
    row("help request", &mixed, &["--help"]);
    row("typo + suggestions", &mixed, &["--jobz", "16", "src", "dst"]);

    let (ns, _) = measure(|| {
        black_box(
            Cmd::new("bench", "1.0.0")
                .flag("verbose", 'v', "")
                .opt("output", 'o', "")
                .done()
                .arg("src", ""),
        );
    });
    println!("{:<24} {:>10.0} {:>14}", "declare 4 args", ns, "-");
}

fn row(label: &str, cmd: &Cmd, argv: &[&str]) {
    let (ns, _) = measure(|| {
        // `parse` owns the argv materialisation, so this is the full cost a CLI
        // pays per invocation, not just the tokenizer. Failure rows keep the
        // rendered error, since building it is what is being measured.
        let out = cmd.parse(argv.iter().copied());
        let _ = black_box(out.map(|m| m.positionals().len()));
    });
    println!("{:<24} {:>10.0} {:>14.0}", label, ns, 1e9 / ns);
}

/// Geometrically calibrate, then time: returns (ns per call, iterations).
fn measure(mut work: impl FnMut()) -> (f64, u64) {
    let mut iters: u64 = 1_000;
    loop {
        let start = Instant::now();
        for _ in 0..iters {
            work();
        }
        let elapsed = start.elapsed();
        if elapsed >= min_window() || iters >= 1 << 26 {
            return (elapsed.as_nanos() as f64 / iters as f64, iters);
        }
        iters *= 8;
    }
}
