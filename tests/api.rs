//! Black-box tests: only what `karg` re-exports is reachable here, so a breakage
//! of the public surface fails even if every unit test still passes.

use karg::{Cmd, Error};

fn tool() -> Cmd {
    Cmd::new("tool", "2.1.0")
        .about("Compress files into an archive")
        .flag("verbose", 'v', "Print each file")
        .opt("output", 'o', "Archive to write")
        .required()
        .done()
        .multi("exclude", 'x', "Glob to skip, repeatable")
        .done()
        .arg("inputs", "Files to add")
        .rest("more", "Additional files")
        .after_help("Report issues at https://example.invalid/tool")
}

#[test]
fn happy_path_reads_back_every_shape() {
    let m = tool()
        .parse([
            "-vvv",
            "--output",
            "out.tar",
            "-x",
            "*.log",
            "-x",
            "target/**",
            "a.txt",
            "b.txt",
            "c.txt",
        ])
        .unwrap();

    assert_eq!(m.command(), "tool");
    assert_eq!(m.count("verbose"), 3);
    assert!(m.flag("verbose"));
    assert_eq!(m.get("output"), Some("out.tar"));
    assert_eq!(m.strings("exclude"), ["*.log", "target/**"]);
    assert_eq!(m.positionals(), ["a.txt", "b.txt", "c.txt"]);
    assert_eq!(m.positional(2), Some("c.txt"));
}

#[test]
fn required_option_absent_is_an_error_with_exit_two() {
    let err = tool().parse(["a.txt"]).unwrap_err();
    assert_eq!(err.kind(), "missing-required");
    assert_eq!(err.exit_code(), 2);
    assert!(err.to_string().contains("output"), "{err}");
}

#[test]
fn typo_suggests_the_nearest_long_option() {
    let err = tool().parse(["--outpu", "x", "a.txt"]).unwrap_err();
    match err {
        Error::UnknownOption { ref given, ref suggestions } => {
            assert_eq!(given, "--outpu");
            assert_eq!(suggestions, &["--output".to_string()]);
        }
        other => panic!("expected UnknownOption, got {other:?}"),
    }
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn double_dash_freezes_the_rest_of_the_line() {
    let m = tool().parse(["-o", "o.tar", "a.txt", "--", "--output", "-v"]).unwrap();
    assert_eq!(m.get("output"), Some("o.tar"));
    assert_eq!(
        m.positionals(),
        ["a.txt", "--output", "-v"],
        "tokens after `--` are positionals, never flags"
    );
    assert!(!m.is_set("verbose"));
}

#[test]
fn defaults_are_visible_but_not_marked_as_set() {
    let cmd = Cmd::new("tool", "1").opt("level", 'l', "").default("3").done();
    let m = cmd.parse(std::iter::empty::<String>()).unwrap();
    assert_eq!(m.get("level"), Some("3"));
    assert!(!m.is_set("level"), "a default is not the user typing it");
    assert!(cmd.parse(["--level", "9"]).unwrap().is_set("level"));
}

#[test]
fn numbers_coerce_and_report_the_offending_token() {
    let cmd = Cmd::new("tool", "1").opt("jobs", 'j', "").done();
    assert_eq!(cmd.parse(["-j", "8"]).unwrap().number::<u8>("jobs"), Ok(Some(8)));
    assert_eq!(cmd.parse(std::iter::empty::<String>()).unwrap().number::<u8>("jobs"), Ok(None));
    // Coercion happens at read time, so the token reaches `number` untouched and
    // the error names both the option and the bad value.
    let m = cmd.parse(["-j", "many"]).unwrap();
    let err = m.number::<u8>("jobs").unwrap_err();
    assert_eq!(err.kind(), "invalid-integer");
    assert!(err.to_string().contains("jobs"), "{err}");
    assert!(err.to_string().contains("many"), "{err}");
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn help_and_version_are_exit_zero_requests() {
    for (arg, needle) in [("--help", "Usage: tool"), ("-V", "tool 2.1.0")] {
        let err = tool().parse([arg]).unwrap_err();
        assert!(err.is_display_request(), "{arg} should print, not fail");
        assert_eq!(err.exit_code(), 0);
        assert!(err.to_string().contains(needle), "{err}");
    }
    let help = tool().help_text();
    assert!(help.contains("-o, --output <OUTPUT>"), "{help}");
    assert!(help.contains("[required]"), "{help}");
    assert!(help.contains("https://example.invalid/tool"), "{help}");
    assert!(
        help.lines().all(|l| l.chars().count() <= 100),
        "help must not overflow a narrow terminal:\n{help}"
    );
}

#[test]
fn negation_and_bare_dash_behave_like_getopt() {
    let cmd = Cmd::new("tool", "1").flag("color", 'c', "");
    assert!(cmd.parse(["--color"]).unwrap().flag("color"));
    let m = cmd.parse(["--no-color"]).unwrap();
    assert!(!m.flag("color"));
    assert!(m.is_set("color"), "an explicit negation is still a choice");

    let cmd = Cmd::new("tool", "1").opt("file", 'f', "").done();
    let m = cmd.parse(["-f", "-"]).unwrap();
    assert_eq!(m.get("file"), Some("-"), "a lone dash is stdin, not a flag");
}

#[test]
fn subcommand_sees_only_its_own_tail() {
    let cmd = Cmd::new("svc", "1").flag("verbose", 'v', "").subcommand(
        Cmd::new("serve", "1").opt("port", 'p', "").default("8080").done().arg("bind", "address"),
    );

    let m = cmd.parse(["-v", "serve", "0.0.0.0", "-p", "9000"]).unwrap();
    assert!(m.flag("verbose"));
    let (name, sub) = m.subcommand().unwrap();
    assert_eq!(name, "serve");
    assert_eq!(sub.get("port"), Some("9000"));
    assert_eq!(sub.positional(0), Some("0.0.0.0"));
    assert!(!sub.is_set("verbose"), "a parent flag must not leak into the child match set");

    // `--help` after a subcommand documents that subcommand, not the root.
    let err = cmd.parse(["serve", "--help"]).unwrap_err();
    assert!(err.to_string().contains("Usage: svc serve"), "{err}");
}

#[test]
fn a_cmd_value_is_reusable_and_send() {
    static ROOT: std::sync::OnceLock<Cmd> = std::sync::OnceLock::new();
    let root = ROOT.get_or_init(tool);
    let threads: Vec<_> = (0..4)
        .map(|_| {
            std::thread::spawn(|| {
                root.parse(["-o", "x", "a"]).unwrap().get("output").map(|s| s.to_string())
            })
        })
        .collect();
    for t in threads {
        assert_eq!(t.join().unwrap().as_deref(), Some("x"));
    }
}
