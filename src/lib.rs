//! # karg
//!
//! A dependency-free command line argument parser: typed accessors, nested
//! subcommands, generated help, and errors that say what to type next.
//!
//! `karg` has **no dependencies** — not even dev-dependencies. Everything here
//! (edit distance, help wrapping, integer coercion) is implemented in-crate, so
//! a `karg` CLI builds from a clean registry cache with nothing to download.
//!
//! ## Quick start
//!
//! ```
//! use karg::Cmd;
//!
//! let cmd = Cmd::new("crop", "1.2.0")
//!     .about("Crop an image to a region")
//!     .flag("verbose", 'v', "Print what is happening")
//!     .opt("format", 'f', "Output format").default("png")
//!     .opt("quality", 'q', "JPEG quality (1-100)")
//!     .multi("tag", 't', "Metadata tag, repeatable")
//!     .arg("source", "Image to read")
//!     .arg("target", "Image to write");
//!
//! let m = cmd.parse(["-vv", "--format=webp", "-q", "82", "a.png", "b.png"]).unwrap();
//!
//! assert_eq!(m.count("verbose"), 2);
//! assert_eq!(m.get("format"), Some("webp"));
//! assert_eq!(m.number_or::<u8>("quality", 90).unwrap(), 82);
//! assert_eq!(m.positional(0), Some("a.png"));
//! assert_eq!(m.positional(1), Some("b.png"));
//! assert_eq!(m.get("tag"), None::<&str>);
//! ```
//!
//! ## Subcommands
//!
//! ```
//! use karg::Cmd;
//!
//! let cmd = Cmd::new("ctl", "0.1.0")
//!     .subcommand(Cmd::new("serve", "0.1.0").opt("port", 'p', "TCP port").default("8080"))
//!     .subcommand(Cmd::new("stop", "0.1.0").flag("all", 'a', "Stop every instance"));
//!
//! let m = cmd.parse(["serve", "--port", "9000"]).unwrap();
//! let (name, sub) = m.subcommand().expect("serve was parsed");
//! assert_eq!(name, "serve");
//! assert_eq!(sub.get("port"), Some("9000"));
//!
//! // Unknown subcommands are rejected against the parent.
//! let err = cmd.parse(["srv"]).unwrap_err();
//! assert_eq!(err.kind(), "unknown-option");
//! ```
//!
//! ## What is deliberately not here
//!
//! * `OsStr`/lossy mode: arguments must be UTF-8, and [`Error::InvalidUtf8`]
//!   says so instead of silently replacing bytes.
//! * Env-var fallback, shell completion, derive macros, unicode width maths.
//!   Each needs a dependency or a build-time code generator; `karg` keeps the
//!   surface small enough to read in one sitting.
//!
//! [`Error::InvalidUtf8`]: crate::Error::InvalidUtf8

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod help;
mod parse;

use std::collections::BTreeMap;
use std::str::FromStr;

pub use error::{Error, Result};

/// A declared option, flag or positional slot.
#[derive(Debug, Clone)]
pub struct ArgSpec {
    /// Long name used as `--long`; positionals keep their placeholder here.
    pub long: String,
    /// Short name used as `-s`, if any.
    pub short: Option<char>,
    /// One-line description shown in `--help`.
    pub help: String,
    /// Whether the token consumes a value (`--out FILE`) or not (`--verbose`).
    pub takes_value: bool,
    /// Placeholder shown in usage, e.g. `FILE`.
    pub value_name: String,
    /// Repeatable: collected into a list instead of overwritten.
    pub multiple: bool,
    /// Reject the parse if this option is absent.
    pub required: bool,
    /// Value used when the option is absent.
    pub default: Option<String>,
    /// Consumes every remaining positional slot.
    pub variadic: bool,
    /// Hide from generated help.
    pub hidden: bool,
}

impl ArgSpec {
    fn new(long: impl Into<String>, short: Option<char>, help: impl Into<String>) -> Self {
        let long = long.into();
        Self {
            value_name: long.to_uppercase(),
            help: help.into(),
            takes_value: false,
            multiple: false,
            required: false,
            default: None,
            variadic: false,
            hidden: false,
            short,
            long,
        }
    }
}

/// A command: the root of a CLI, or one of its subcommands.
///
/// Built by chaining [`Cmd::new`] with `flag`/`opt`/`multi`/`arg`/`subcommand`,
/// then frozen and reused for parsing. All builder methods consume and return
/// `self`, so a command is normally one expression.
#[derive(Debug, Clone)]
pub struct Cmd {
    name: String,
    version: String,
    about: String,
    after_help: String,
    opts: Vec<ArgSpec>,
    positionals: Vec<ArgSpec>,
    subs: Vec<Cmd>,
    subcommand_required: bool,
}

impl Cmd {
    /// Start a command named `name` with a `--version` string.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            about: String::new(),
            after_help: String::new(),
            opts: Vec::new(),
            positionals: Vec::new(),
            subs: Vec::new(),
            subcommand_required: false,
        }
    }

    /// One-line summary used in `--help` and in parent subcommand listings.
    pub fn about(mut self, text: impl Into<String>) -> Self {
        self.about = text.into();
        self
    }

    /// Free-form block appended below the option table.
    pub fn after_help(mut self, text: impl Into<String>) -> Self {
        self.after_help = text.into();
        self
    }

    /// Add a boolean flag: `--long` / `-s`, no value, counts occurrences.
    pub fn flag(mut self, long: impl Into<String>, short: char, help: impl Into<String>) -> Self {
        self.opts.push(ArgSpec::new(long, Some(short), help));
        self
    }

    /// Same as [`Cmd::flag`] but without a short alias.
    pub fn long_flag(mut self, long: impl Into<String>, help: impl Into<String>) -> Self {
        self.opts.push(ArgSpec::new(long, None, help));
        self
    }

    /// Start an option that takes a value: `--long VALUE` / `-s VALUE`.
    ///
    /// Chain [`ArgBuilder::default`], [`ArgBuilder::required`],
    /// [`ArgBuilder::value_name`] and [`ArgBuilder::hidden`] on the result.
    pub fn opt(self, long: impl Into<String>, short: char, help: impl Into<String>) -> ArgBuilder {
        let mut spec = ArgSpec::new(long, Some(short), help);
        spec.takes_value = true;
        ArgBuilder { cmd: self, spec }
    }

    /// Start a repeatable option; each occurrence appends to the list.
    pub fn multi(
        self,
        long: impl Into<String>,
        short: char,
        help: impl Into<String>,
    ) -> ArgBuilder {
        let mut spec = ArgSpec::new(long, Some(short), help);
        spec.takes_value = true;
        spec.multiple = true;
        ArgBuilder { cmd: self, spec }
    }

    /// Start a value-taking option with no short alias.
    pub fn long_opt(self, long: impl Into<String>, help: impl Into<String>) -> ArgBuilder {
        let mut spec = ArgSpec::new(long, None, help);
        spec.takes_value = true;
        ArgBuilder { cmd: self, spec }
    }

    /// Declare a required positional slot.
    pub fn arg(mut self, name: impl Into<String>, help: impl Into<String>) -> Self {
        let mut spec = ArgSpec::new(name, None, help);
        spec.required = true;
        self.positionals.push(spec);
        self
    }

    /// Declare an optional positional slot.
    pub fn opt_arg(mut self, name: impl Into<String>, help: impl Into<String>) -> Self {
        self.positionals.push(ArgSpec::new(name, None, help));
        self
    }

    /// Declare a trailing positional that swallows every remaining argument.
    ///
    /// Only the last slot may be variadic.
    pub fn rest(mut self, name: impl Into<String>, help: impl Into<String>) -> Self {
        let mut spec = ArgSpec::new(name, None, help);
        spec.variadic = true;
        self.positionals.push(spec);
        self
    }

    /// Attach a subcommand.
    pub fn subcommand<S: Into<Cmd>>(mut self, sub: S) -> Self {
        self.subs.push(sub.into());
        self
    }

    /// Fail with [`Error::SubcommandRequired`] when no subcommand is given.
    pub fn require_subcommand(mut self) -> Self {
        self.subcommand_required = true;
        self
    }

    /// Command name as registered with [`Cmd::new`].
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Version string as registered with [`Cmd::new`].
    pub fn get_version(&self) -> &str {
        &self.version
    }

    /// Declared subcommands, in registration order.
    pub fn subcommands(&self) -> &[Cmd] {
        &self.subs
    }
}

/// Fluent tail of an [`Cmd::opt`]-style declaration.
#[derive(Debug)]
pub struct ArgBuilder {
    cmd: Cmd,
    spec: ArgSpec,
}

impl ArgBuilder {
    /// Use `v` when the option is absent.
    pub fn default(mut self, v: impl Into<String>) -> Self {
        self.spec.default = Some(v.into());
        self
    }

    /// Reject the parse when the option is absent.
    pub fn required(mut self) -> Self {
        self.spec.required = true;
        self
    }

    /// Override the usage placeholder (`FILE` rather than the long name).
    pub fn value_name(mut self, v: impl Into<String>) -> Self {
        self.spec.value_name = v.into();
        self
    }

    /// Keep the option out of generated help while still accepting it.
    pub fn hidden(mut self) -> Self {
        self.spec.hidden = true;
        self
    }

    /// Finish and return the command.
    pub fn done(self) -> Cmd {
        let ArgBuilder { mut cmd, spec } = self;
        cmd.opts.push(spec);
        cmd
    }

    // Everything below commits this option and continues the chain on `Cmd`,
    // so a declaration can end at any point without an explicit `done()`.
    fn then<T>(self, next: impl FnOnce(Cmd) -> T) -> T {
        next(self.done())
    }

    /// Commit this option, then add a flag.
    pub fn flag(self, long: impl Into<String>, short: char, help: impl Into<String>) -> Cmd {
        self.then(|c| c.flag(long, short, help))
    }

    /// Commit this option, then add a long-only flag.
    pub fn long_flag(self, long: impl Into<String>, help: impl Into<String>) -> Cmd {
        self.then(|c| c.long_flag(long, help))
    }

    /// Commit this option, then add another value-taking option.
    pub fn opt(self, long: impl Into<String>, short: char, help: impl Into<String>) -> ArgBuilder {
        self.then(|c| c.opt(long, short, help))
    }

    /// Commit this option, then add a repeatable option.
    pub fn multi(
        self,
        long: impl Into<String>,
        short: char,
        help: impl Into<String>,
    ) -> ArgBuilder {
        self.then(|c| c.multi(long, short, help))
    }

    /// Commit this option, then add a long-only value option.
    pub fn long_opt(self, long: impl Into<String>, help: impl Into<String>) -> ArgBuilder {
        self.then(|c| c.long_opt(long, help))
    }

    /// Commit this option, then declare a required positional.
    pub fn arg(self, name: impl Into<String>, help: impl Into<String>) -> Cmd {
        self.then(|c| c.arg(name, help))
    }

    /// Commit this option, then declare an optional positional.
    pub fn opt_arg(self, name: impl Into<String>, help: impl Into<String>) -> Cmd {
        self.then(|c| c.opt_arg(name, help))
    }

    /// Commit this option, then declare a trailing variadic positional.
    pub fn rest(self, name: impl Into<String>, help: impl Into<String>) -> Cmd {
        self.then(|c| c.rest(name, help))
    }

    /// Commit this option, then attach a subcommand.
    pub fn subcommand<S: Into<Cmd>>(self, sub: S) -> Cmd {
        self.then(|c| c.subcommand(sub))
    }

    /// Commit this option, then set the summary line.
    pub fn about(self, text: impl Into<String>) -> Cmd {
        self.then(|c| c.about(text))
    }

    /// Commit this option, then append a help epilogue.
    pub fn after_help(self, text: impl Into<String>) -> Cmd {
        self.then(|c| c.after_help(text))
    }

    /// Commit this option, then require a subcommand.
    pub fn require_subcommand(self) -> Cmd {
        self.then(Cmd::require_subcommand)
    }

    // Parsing entry points, so a chain can be used without naming `done()`.
    // Each commits this option first; reuse the returned `Cmd` if you parse in
    // a loop.

    /// Commit this option and parse `args`.
    pub fn parse<I, S>(self, args: I) -> Result<Matches>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.done().parse(args)
    }

    /// Commit this option and parse the process arguments.
    pub fn parse_std(self) -> Result<Matches> {
        self.done().parse_std()
    }

    /// Commit this option, parse the process arguments and handle errors.
    #[must_use]
    pub fn parse_or_exit(self) -> Matches {
        self.done().parse_or_exit()
    }

    /// Commit this option and render its `--help`.
    pub fn help_text(self) -> String {
        self.done().help_text()
    }
}

impl From<ArgBuilder> for Cmd {
    fn from(builder: ArgBuilder) -> Self {
        builder.done()
    }
}

/// An option's captured values plus whether the user actually typed it.
#[derive(Debug, Clone, Default)]
struct Occurrence {
    values: Vec<String>,
    explicit: bool,
}

/// Parse output: query it by long name or by positional index.
#[derive(Debug, Clone, Default)]
pub struct Matches {
    name: String,
    given: BTreeMap<String, Occurrence>,
    positionals: Vec<String>,
    sub: Option<(String, Box<Matches>)>,
}

impl Matches {
    /// Name of the command these matches belong to (the deepest subcommand).
    pub fn command(&self) -> &str {
        &self.name
    }

    /// Was `name` typed on the command line (as opposed to filled from a default)?
    pub fn is_set(&self, name: &str) -> bool {
        self.given.get(name).is_some_and(|o| o.explicit)
    }

    /// `true` for a boolean flag that was enabled.
    ///
    /// `--no-flag` registers the flag as present-but-disabled, so `flag`
    /// returns `false` while [`Matches::is_set`] returns `true`.
    pub fn flag(&self, name: &str) -> bool {
        match self.given.get(name) {
            Some(o) => o.values.last().map(String::as_str) != Some("false"),
            None => false,
        }
    }

    /// How many times a flag or repeatable option appeared.
    pub fn count(&self, name: &str) -> usize {
        self.given.get(name).map_or(0, |o| o.values.len())
    }

    /// Last value supplied for `name`, or its default.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.given.get(name)?.values.last().map(String::as_str)
    }

    /// Last value for `name`, falling back to `fallback` when absent.
    pub fn get_or<'a>(&'a self, name: &str, fallback: &'a str) -> &'a str {
        self.get(name).unwrap_or(fallback)
    }

    /// Every value of a repeatable option, in the order typed.
    pub fn strings(&self, name: &str) -> &[String] {
        self.given.get(name).map_or(&[][..], |o| &o.values[..])
    }

    /// Parse `name` as `T`. `Ok(None)` means the option was absent.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInteger`] when the text does not parse as `T`.
    pub fn number<T: FromStr>(&self, name: &str) -> Result<Option<T>> {
        match self.get(name) {
            None => Ok(None),
            Some(raw) => raw.parse::<T>().map(Some).map_err(|_| Error::InvalidInteger {
                option: name.to_string(),
                value: raw.to_string(),
                kind: std::any::type_name::<T>(),
            }),
        }
    }

    /// Parse `name` as `T`, using `fallback` when the option is absent.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInteger`] when the text does not parse as `T`.
    pub fn number_or<T: FromStr + Clone>(&self, name: &str, fallback: T) -> Result<T> {
        Ok(self.number::<T>(name)?.unwrap_or(fallback))
    }

    /// Positional argument at `index` (0-based).
    pub fn positional(&self, index: usize) -> Option<&str> {
        self.positionals.get(index).map(String::as_str)
    }

    /// All positional arguments in command line order.
    pub fn positionals(&self) -> &[String] {
        &self.positionals
    }

    /// The parsed subcommand as `(name, matches)`.
    pub fn subcommand(&self) -> Option<(&str, &Matches)> {
        self.sub.as_ref().map(|(n, m)| (n.as_str(), &**m))
    }

    fn insert(&mut self, name: &str, value: String) {
        let occ = self.given.entry(name.to_string()).or_default();
        occ.values.push(value);
        occ.explicit = true;
    }

    fn insert_negative(&mut self, name: &str) {
        let occ = self.given.entry(name.to_string()).or_default();
        occ.values.push("false".to_string());
        occ.explicit = true;
    }

    fn apply_defaults(&mut self, cmd: &Cmd) {
        for spec in &cmd.opts {
            if self.given.contains_key(&spec.long) {
                continue;
            }
            if let Some(default) = &spec.default {
                let occ = self.given.entry(spec.long.clone()).or_default();
                occ.values.push(default.clone());
                occ.explicit = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Cmd {
        Cmd::new("sample", "9.9.9")
            .about("fixture")
            .flag("watch", 'w', "Re-run on change")
            .opt("jobs", 'j', "Parallelism")
            .default("4")
            .value_name("N")
            .multi("define", 'D', "Key=value pairs")
            .arg("src", "Input file")
            .opt_arg("dst", "Output file")
    }

    #[test]
    fn defaults_apply_without_marking_is_set() {
        let m = sample().parse(["-D", "A=1", "in.rs"]).unwrap();
        assert_eq!(m.get("jobs"), Some("4"));
        assert!(!m.is_set("jobs"), "defaults must not look explicit");
        assert!(m.is_set("define"));
        assert_eq!(m.strings("define"), ["A=1"]);
    }

    #[test]
    fn negating_a_flag_keeps_it_explicit() {
        let m = sample().parse(["--no-watch", "in.rs"]).unwrap();
        assert!(!m.flag("watch"));
        assert!(m.is_set("watch"));
    }

    #[test]
    fn missing_required_positional_is_reported() {
        let err = sample().parse(["--jobs=2"]).unwrap_err();
        assert_eq!(err.kind(), "missing-required");
        assert!(err.to_string().contains("<src>"), "{err}");
    }

    #[test]
    fn builder_exposes_identity() {
        let cmd = sample();
        assert_eq!(cmd.get_name(), "sample");
        assert_eq!(cmd.get_version(), "9.9.9");
        assert!(cmd.subcommands().is_empty());
    }
}
