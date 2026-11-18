//! The tokenizer: how a `Cmd` turns `argv` into `Matches`.

use crate::error::Error;
use crate::help::render_help;
use crate::{ArgSpec, Cmd, Matches, Result};

impl Cmd {
    /// Parse an argument list (without the program name).
    ///
    /// # Errors
    ///
    /// Any [`Error`] variant except [`Error::InvalidUtf8`]. [`Error::Help`] and
    /// [`Error::Version`] are returned when `--help`/`--version` is requested,
    /// carrying the text to print.
    ///
    /// # Examples
    ///
    /// ```
    /// use karg::Cmd;
    /// let cmd = Cmd::new("x", "0.0.1").flag("dry", 'n', "Preview only");
    /// assert!(cmd.parse(["--dry"]).unwrap().flag("dry"));
    /// assert!(cmd.parse(["--nope"]).unwrap_err().to_string().contains("unexpected argument"));
    /// ```
    pub fn parse<I, S>(&self, args: I) -> Result<Matches>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let argv: Vec<String> = args.into_iter().map(Into::into).collect();
        let mut cursor = 0usize;
        self.parse_one(&argv, &mut cursor, &self.name.clone())
    }

    /// Parse the real process arguments, skipping `argv[0]`.
    ///
    /// # Errors
    ///
    /// See [`Cmd::parse`], plus [`Error::InvalidUtf8`] for non-UTF-8 arguments.
    pub fn parse_std(&self) -> Result<Matches> {
        let mut argv = Vec::new();
        for raw in std::env::args_os().skip(1) {
            match raw.into_string() {
                Ok(s) => argv.push(s),
                Err(os) => {
                    return Err(Error::InvalidUtf8 { given: os.to_string_lossy().into_owned() })
                }
            }
        }
        self.parse(argv)
    }

    /// [`Cmd::parse_std`], but help/version/errors are printed and the process
    /// exits with the code from [`Error::exit_code`].
    #[must_use]
    pub fn parse_or_exit(&self) -> Matches {
        match self.parse_std() {
            Ok(m) => m,
            Err(err) => {
                let text = err.to_string();
                if err.is_display_request() {
                    println!("{text}");
                    std::process::exit(0);
                }
                eprintln!("{}: error: {text}", self.name);
                eprintln!("Try '{} --help' for usage.", self.name);
                std::process::exit(err.exit_code());
            }
        }
    }

    /// Rendered `--help` output for this command.
    pub fn help_text(&self) -> String {
        render_help(self, &self.name.clone())
    }

    /// Fully parsed matches for this command and its subcommands.
    fn parse_one(&self, argv: &[String], cursor: &mut usize, path: &str) -> Result<Matches> {
        let mut m = Matches { name: self.name.clone(), ..Matches::default() };
        let mut pos_idx = 0usize;

        while *cursor < argv.len() {
            let token = argv[*cursor].clone();

            if token == "--" {
                *cursor += 1;
                while *cursor < argv.len() {
                    self.push_positional(&mut m, &mut pos_idx, argv[*cursor].clone())?;
                    *cursor += 1;
                }
                break;
            }

            if token.starts_with("--") && token.len() > 2 {
                *cursor += 1;
                let body = token[2..].to_string();
                let (name, inline) = match body.find('=') {
                    Some(split) => (&body[..split], Some(body[split + 1..].to_string())),
                    None => (body.as_str(), None),
                };

                if name == "help" {
                    return Err(Error::Help(render_help(self, path)));
                }
                if name == "version" {
                    return Err(Error::Version(format!("{path} {}", self.version)));
                }

                match self.find_long(name) {
                    Some(spec) if spec.takes_value => {
                        let value = match inline {
                            Some(v) => v,
                            None => self.take_next(argv, cursor, spec)?,
                        };
                        m.insert(&spec.long, value);
                    }
                    Some(spec) => match inline {
                        None => m.insert(&spec.long, "true".to_string()),
                        Some(v) if v == "true" => m.insert(&spec.long, "true".to_string()),
                        Some(v) if v == "false" => m.insert_negative(&spec.long),
                        Some(v) => {
                            return Err(Error::UnexpectedValue {
                                option: spec.long.clone(),
                                value: v,
                            })
                        }
                    },
                    None => {
                        if let Some(base) = name.strip_prefix("no-") {
                            if let Some(spec) = self.find_long(base) {
                                if spec.takes_value {
                                    return Err(Error::InvalidNegation {
                                        option: spec.long.clone(),
                                    });
                                }
                                m.insert_negative(&spec.long);
                                continue;
                            }
                        }
                        return Err(Error::UnknownOption {
                            // Point at the flag, not the whole `--flag=value`.
                            given: inline.map_or(token, |_| format!("--{name}")),
                            suggestions: self.suggest_long(name),
                        });
                    }
                }
                continue;
            }

            if token.starts_with('-') && token.len() > 1 {
                *cursor += 1;
                let chars: Vec<char> = token.chars().skip(1).collect();
                let mut k = 0usize;
                while k < chars.len() {
                    let c = chars[k];
                    if self.find_short(c).is_none() {
                        if c == 'h' {
                            return Err(Error::Help(render_help(self, path)));
                        }
                        if c == 'V' {
                            return Err(Error::Version(format!("{path} {}", self.version)));
                        }
                        return Err(Error::UnknownOption {
                            given: format!("-{c}"),
                            suggestions: self.suggest_short(c),
                        });
                    }
                    let spec = self.find_short(c).expect("checked above");
                    if spec.takes_value {
                        let tail: String = chars[k + 1..].iter().collect();
                        let tail = tail.strip_prefix('=').map_or(tail.clone(), str::to_string);
                        let value = if tail.is_empty() {
                            self.take_next(argv, cursor, spec)?
                        } else {
                            tail
                        };
                        m.insert(&spec.long, value);
                        break;
                    }
                    m.insert(&spec.long, "true".to_string());
                    k += 1;
                }
                continue;
            }

            // Not an option: either a subcommand or a positional.
            if let Some(sub) = self.subs.iter().find(|c| c.name == token) {
                let sub_path = format!("{path} {}", sub.name);
                let mut inner = *cursor + 1;
                let sm = sub.parse_one(argv, &mut inner, &sub_path)?;
                m.sub = Some((token.clone(), Box::new(sm)));
                *cursor = inner;
                break;
            }

            if !self.subs.is_empty() && self.positionals.is_empty() {
                // No positional slots exist, so a bare token can only be a
                // (misspelled) subcommand.
                let available: Vec<String> = self.subs.iter().map(|c| c.name.clone()).collect();
                let suggestions = nearest(&token, &available);
                return Err(Error::UnknownOption { given: token, suggestions });
            }

            self.push_positional(&mut m, &mut pos_idx, token)?;
            *cursor += 1;
        }

        if self.subcommand_required && m.sub.is_none() {
            return Err(Error::SubcommandRequired {
                available: self.subs.iter().map(|c| c.name.clone()).collect(),
            });
        }

        m.apply_defaults(self);
        self.validate(&m)?;
        Ok(m)
    }

    fn take_next(&self, argv: &[String], cursor: &mut usize, spec: &ArgSpec) -> Result<String> {
        if *cursor < argv.len() {
            let value = argv[*cursor].clone();
            *cursor += 1;
            Ok(value)
        } else {
            Err(Error::MissingValue { option: spec.long.clone() })
        }
    }

    fn push_positional(&self, m: &mut Matches, pos_idx: &mut usize, value: String) -> Result<()> {
        match self.positionals.get(*pos_idx) {
            Some(spec) if spec.variadic => {
                m.insert(&spec.long, value.clone());
                m.positionals.push(value);
            }
            Some(spec) => {
                m.insert(&spec.long, value.clone());
                m.positionals.push(value);
                *pos_idx += 1;
            }
            None => return Err(Error::TooManyPositionals { index: *pos_idx, given: value }),
        }
        Ok(())
    }

    fn validate(&self, m: &Matches) -> Result<()> {
        for spec in &self.opts {
            if spec.required && m.get(&spec.long).is_none() {
                return Err(Error::MissingRequired { reference: format!("--{}", spec.long) });
            }
        }
        for spec in &self.positionals {
            if spec.required && m.get(&spec.long).is_none() {
                return Err(Error::MissingRequired { reference: format!("<{}>", spec.long) });
            }
        }
        Ok(())
    }

    pub(crate) fn find_long(&self, name: &str) -> Option<&ArgSpec> {
        self.opts.iter().find(|o| o.long == name)
    }

    pub(crate) fn find_short(&self, name: char) -> Option<&ArgSpec> {
        self.opts.iter().find(|o| o.short == Some(name))
    }

    fn suggest_long(&self, given: &str) -> Vec<String> {
        let pool: Vec<String> = self
            .opts
            .iter()
            .filter(|o| !o.hidden)
            .flat_map(|o| {
                let mut names = vec![o.long.clone()];
                if !o.takes_value {
                    names.push(format!("no-{}", o.long));
                }
                names
            })
            .collect();
        nearest(given, &pool).into_iter().map(|name| format!("--{name}")).collect()
    }

    fn suggest_short(&self, given: char) -> Vec<String> {
        let pool: Vec<String> = self
            .opts
            .iter()
            .filter(|o| !o.hidden)
            .filter_map(|o| o.short)
            .map(|c| c.to_string())
            .collect();
        let needle = given.to_string();
        let mut out = Vec::new();
        for cand in nearest_all(&needle, &pool) {
            if out.len() == 3 {
                break;
            }
            out.push(format!("-{cand}"));
        }
        out
    }
}

/// Names from `pool` within edit distance 2 of `needle`, closest first.
pub(crate) fn nearest(needle: &str, pool: &[String]) -> Vec<String> {
    let mut scored: Vec<(usize, &String)> = pool
        .iter()
        .filter_map(|cand| {
            let bare = cand.trim_start_matches('-');
            let dist = edit_distance(needle, bare);
            (dist <= 2).then_some((dist, cand))
        })
        .collect();
    scored.sort_by_key(|(dist, cand)| (*dist, cand.as_str()));
    scored.into_iter().take(3).map(|(_, c)| c.clone()).collect()
}

fn nearest_all(needle: &str, pool: &[String]) -> Vec<String> {
    let mut scored: Vec<(usize, &String)> = pool
        .iter()
        .filter_map(|cand| {
            let dist = edit_distance(needle, cand);
            (dist <= 2).then_some((dist, cand))
        })
        .collect();
    scored.sort_by_key(|(dist, cand)| (*dist, cand.as_str()));
    scored.into_iter().map(|(_, c)| c.clone()).collect()
}

/// Levenshtein distance, two-row, no allocations beyond the rows themselves.
pub(crate) fn edit_distance(a: &str, b: &str) -> usize {
    let left: Vec<char> = a.chars().collect();
    let right: Vec<char> = b.chars().collect();
    if left.is_empty() {
        return right.len();
    }
    if right.is_empty() {
        return left.len();
    }
    let mut prev: Vec<usize> = (0..=right.len()).collect();
    let mut cur = vec![0usize; right.len() + 1];
    for (i, &lc) in left.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &rc) in right.iter().enumerate() {
            let cost = usize::from(lc != rc);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_basics() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("abc", "abc"), 0);
        assert_eq!(edit_distance("colour", "color"), 1);
        assert_eq!(edit_distance("port", "prt"), 1);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn clustered_shorts_split_correctly() {
        let cmd =
            Cmd::new("c", "1").flag("alpha", 'a', "").flag("beta", 'b', "").opt("jobs", 'j', "");
        let m = cmd.parse(["-abj", "8"]).unwrap();
        assert!(m.flag("alpha"));
        assert!(m.flag("beta"));
        assert_eq!(m.get("jobs"), Some("8"));
    }

    #[test]
    fn value_may_attach_to_short() {
        let cmd = Cmd::new("c", "1").opt("jobs", 'j', "").arg("file", "input");
        assert_eq!(cmd.parse(["-j8", "x"]).unwrap().get("jobs"), Some("8"));
        assert_eq!(cmd.parse(["-j=16", "x"]).unwrap().get("jobs"), Some("16"));
        // A trailing short that takes a value consumes the next token instead.
        let m = cmd.parse(["-j", "32", "x"]).unwrap();
        assert_eq!(m.get("jobs"), Some("32"));
        assert_eq!(m.positional(0), Some("x"));
    }

    #[test]
    fn double_dash_stops_option_parsing() {
        let cmd = Cmd::new("c", "1").flag("force", 'f', "").rest("args", "passthrough");
        let m = cmd.parse(["--force", "--", "--nope", "-x"]).unwrap();
        assert!(m.flag("force"));
        assert_eq!(m.strings("args"), ["--nope", "-x"]);
        assert_eq!(m.positionals(), ["--nope", "-x"]);
    }

    #[test]
    fn last_value_wins_for_single_options() {
        let cmd = Cmd::new("c", "1").opt("level", 'l', "");
        let m = cmd.parse(["--level=1", "--level", "9"]).unwrap();
        assert_eq!(m.get("level"), Some("9"));
        assert_eq!(m.count("level"), 2);
    }

    #[test]
    fn unknown_long_suggests_declared_one() {
        let cmd = Cmd::new("c", "1").opt("color", 'c', "");
        let err = cmd.parse(["--colour", "red"]).unwrap_err();
        match err {
            Error::UnknownOption { given, suggestions } => {
                assert_eq!(given, "--colour");
                assert_eq!(suggestions, vec!["--color".to_string()]);
            }
            other => panic!("expected UnknownOption, got {other:?}"),
        }
    }

    #[test]
    fn unknown_flag_with_inline_value_blames_the_flag() {
        let cmd = Cmd::new("c", "1").opt("format", 'f', "");
        match cmd.parse(["--formt=webp"]).unwrap_err() {
            Error::UnknownOption { given, suggestions } => {
                assert_eq!(given, "--formt", "the value is not what went wrong");
                assert_eq!(suggestions, vec!["--format".to_string()]);
            }
            other => panic!("expected UnknownOption, got {other:?}"),
        }
    }

    #[test]
    fn hidden_options_parse_but_do_not_suggest() {
        let cmd = Cmd::new("c", "1").long_opt("trace", "internal").hidden().done();
        assert_eq!(cmd.parse(["--trace=on"]).unwrap().get("trace"), Some("on"));
        let err = cmd.parse(["--trac"]).unwrap_err();
        match err {
            Error::UnknownOption { suggestions, .. } => assert!(suggestions.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn missing_value_error_names_the_option() {
        let cmd = Cmd::new("c", "1").opt("output", 'o', "").done();
        assert_eq!(
            cmd.parse(["a.txt"]).unwrap_err(),
            Error::TooManyPositionals { index: 0, given: "a.txt".into() },
            "a surplus positional is reported before unrelated flags"
        );
        assert_eq!(
            cmd.parse(["--output"]).unwrap_err(),
            Error::MissingValue { option: "output".into() }
        );
        assert_eq!(cmd.parse(["-o"]).unwrap_err(), Error::MissingValue { option: "output".into() });
    }

    #[test]
    fn bare_dash_is_a_positional() {
        let cmd = Cmd::new("c", "1").opt_arg("file", "input");
        assert_eq!(cmd.parse(["-"]).unwrap().positional(0), Some("-"));
    }

    #[test]
    fn cursor_is_advanced_past_consumed_values() {
        let cmd = Cmd::new("c", "1").opt("name", 'n', "").arg("rest", "leftover");
        let m = cmd.parse(["--name", "value", "tail"]).unwrap();
        assert_eq!(m.get("name"), Some("value"));
        assert_eq!(m.positional(0), Some("tail"));
    }

    #[test]
    fn subcommand_receives_only_its_own_tail() {
        let cmd = Cmd::new("top", "1")
            .flag("verbose", 'v', "")
            .subcommand(Cmd::new("serve", "1").opt("port", 'p', "").arg("host", "bind"));
        let m = cmd.parse(["--verbose", "serve", "--port", "80", "0.0.0.0"]).unwrap();
        assert!(m.flag("verbose"));
        let (name, sub) = m.subcommand().unwrap();
        assert_eq!(name, "serve");
        assert_eq!(sub.get("port"), Some("80"));
        assert_eq!(sub.positional(0), Some("0.0.0.0"));
        assert!(!sub.flag("verbose"), "parent flags stay on the parent");
    }

    #[test]
    fn nested_subcommands_work() {
        let cmd = Cmd::new("top", "1").subcommand(
            Cmd::new("remote", "1")
                .subcommand(Cmd::new("add", "1").arg("name", "").arg("url", ""))
                .subcommand(Cmd::new("remove", "1").arg("name", "")),
        );
        let m = cmd.parse(["remote", "add", "origin", "https://x.test"]).unwrap();
        let (_, remote) = m.subcommand().unwrap();
        let (_, add) = remote.subcommand().unwrap();
        assert_eq!(add.positional(0), Some("origin"));
        assert_eq!(add.positional(1), Some("https://x.test"));
    }

    #[test]
    fn require_subcommand_rejects_empty_invocation() {
        let cmd = Cmd::new("tool", "1").subcommand(Cmd::new("run", "1")).require_subcommand();
        let err = cmd.parse(Vec::<String>::new()).unwrap_err();
        assert_eq!(err.kind(), "subcommand-required");
        assert!(err.to_string().contains("run"));
    }

    #[test]
    fn unknown_subcommand_is_rejected_with_candidates() {
        let cmd = Cmd::new("tool", "1")
            .subcommand(Cmd::new("serve", "1"))
            .subcommand(Cmd::new("watch", "1"));
        let err = cmd.parse(["srv"]).unwrap_err();
        match err {
            Error::UnknownOption { given, suggestions } => {
                assert_eq!(given, "srv");
                assert!(suggestions.contains(&"serve".to_string()), "{suggestions:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn too_many_positionals_reports_index() {
        let cmd = Cmd::new("c", "1").arg("a", "");
        let err = cmd.parse(["one", "two"]).unwrap_err();
        match err {
            Error::TooManyPositionals { index, given } => {
                assert_eq!(index, 1);
                assert_eq!(given, "two");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn help_and_version_are_requests_not_failures() {
        let cmd = Cmd::new("c", "2.1").flag("x", 'X', "ex");
        let help = cmd.parse(["-h"]).unwrap_err();
        assert!(help.is_display_request());
        assert_eq!(help.exit_code(), 0);
        assert!(help.to_string().contains("--x"));
        let version = cmd.parse(["--version"]).unwrap_err();
        assert_eq!(version.to_string(), "c 2.1");
    }

    #[test]
    fn help_uses_the_invocation_path() {
        let cmd = Cmd::new("tool", "1").subcommand(Cmd::new("sync", "1").flag("force", 'f', ""));
        let err = cmd.parse(["sync", "--help"]).unwrap_err();
        assert!(err.to_string().starts_with("Usage: tool sync"), "{err}");
    }

    #[test]
    fn explicit_short_beats_builtin_help() {
        let cmd = Cmd::new("c", "1").flag("hard", 'h', "custom");
        assert!(cmd.parse(["-h"]).unwrap().flag("hard"));
    }

    #[test]
    fn invalid_utf8_error_is_printable() {
        // parse_std reads the real process args, and injecting invalid UTF-8
        // into this test's argv is not portable across platforms, so the
        // rendering contract is asserted directly.
        let err = Error::InvalidUtf8 {
            given: String::from_utf8_lossy(&[b'b', b'a', b'd', 0xFF]).into_owned(),
        };
        assert!(err.to_string().contains("not valid UTF-8"), "{err}");
        assert_eq!(err.kind(), "invalid-utf8");
    }
}
