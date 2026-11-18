//! Every failure mode `karg` can report, plus the exit code it maps to.

use std::fmt;

/// Result alias used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Argument parsing failures.
///
/// [`Error::Help`] and [`Error::Version`] are *not* failures: they carry the
/// rendered text and map to exit status `0`. Everything else maps to `2`,
/// matching the convention used by `git`, `cargo` and GNU coreutils.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A `--long` or `-s` token that the active command does not declare.
    UnknownOption {
        /// The token exactly as it appeared on the command line.
        given: String,
        /// Near-miss option names, closest first (at most 3).
        suggestions: Vec<String>,
    },
    /// `--out` (or `-o`) was given but nothing followed it.
    MissingValue {
        /// Canonical long name, e.g. `output`.
        option: String,
    },
    /// A boolean flag received `=value` that is not `true`/`false`.
    UnexpectedValue {
        /// Canonical long name.
        option: String,
        /// The offending value.
        value: String,
    },
    /// `--no-flag` was used for something that is not a boolean flag.
    InvalidNegation {
        /// Canonical long name.
        option: String,
    },
    /// A required option or positional was absent.
    MissingRequired {
        /// Human-readable reference, e.g. `--output` or `<path>`.
        reference: String,
    },
    /// More positional arguments than the command declares.
    TooManyPositionals {
        /// Zero-based index of the surplus argument.
        index: usize,
        /// The surplus value.
        given: String,
    },
    /// A value that must be an integer was not, or did not fit the type.
    InvalidInteger {
        /// Option or positional the value belongs to.
        option: String,
        /// Raw text supplied by the user.
        value: String,
        /// Name of the numeric type, e.g. `u32`.
        kind: &'static str,
    },
    /// An argument was not valid UTF-8.
    InvalidUtf8 {
        /// Lossy rendering, so the message stays printable.
        given: String,
    },
    /// A subcommand name was expected but missing or unknown.
    SubcommandRequired {
        /// Declared subcommand names.
        available: Vec<String>,
    },
    /// Print this and exit 0.
    Help(String),
    /// Print this and exit 0.
    Version(String),
}

impl Error {
    /// Exit status a CLI should use for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Help(_) | Error::Version(_) => 0,
            _ => 2,
        }
    }

    /// `true` for [`Error::Help`] and [`Error::Version`].
    pub fn is_display_request(&self) -> bool {
        matches!(self, Error::Help(_) | Error::Version(_))
    }

    /// Short, stable label for the variant, useful in tests and logs.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::UnknownOption { .. } => "unknown-option",
            Error::MissingValue { .. } => "missing-value",
            Error::UnexpectedValue { .. } => "unexpected-value",
            Error::InvalidNegation { .. } => "invalid-negation",
            Error::MissingRequired { .. } => "missing-required",
            Error::TooManyPositionals { .. } => "too-many-positionals",
            Error::InvalidInteger { .. } => "invalid-integer",
            Error::InvalidUtf8 { .. } => "invalid-utf8",
            Error::SubcommandRequired { .. } => "subcommand-required",
            Error::Help(_) => "help",
            Error::Version(_) => "version",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownOption { given, suggestions } => {
                write!(f, "unexpected argument '{given}'")?;
                match suggestions.as_slice() {
                    [] => Ok(()),
                    [one] => write!(f, ", did you mean '{one}'?"),
                    many => write!(f, ", did you mean one of: {}?", join(many)),
                }
            }
            Error::MissingValue { option } => {
                write!(f, "option '--{option}' requires a value")
            }
            Error::UnexpectedValue { option, value } => write!(
                f,
                "'--{option}' is a flag and takes no value, but '{value}' was supplied \
                 (use '--{option}' alone to enable it, '--no-{option}' to disable it)"
            ),
            Error::InvalidNegation { option } => write!(
                f,
                "'--no-{option}' is only valid for boolean flags, but '--{option}' takes a value"
            ),
            Error::MissingRequired { reference } => {
                write!(f, "the required argument {reference} was not provided")
            }
            Error::TooManyPositionals { index, given } => write!(
                f,
                "unexpected positional argument #{index} ('{given}'); this command takes fewer arguments"
            ),
            Error::InvalidInteger { option, value, kind } => {
                write!(f, "expected '{option}' to be a valid {kind}, got '{value}'")
            }
            Error::InvalidUtf8 { given } => {
                write!(f, "argument '{given}' is not valid UTF-8; this tool requires UTF-8 input")
            }
            Error::SubcommandRequired { available } => write!(
                f,
                "a subcommand is required, expected one of: {}",
                join(available)
            ),
            Error::Help(text) | Error::Version(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for Error {}

fn join(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        many => {
            let (head, last) = many.split_at(many.len() - 1);
            format!("{} or {}", head.join(", "), last[0])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_are_listed_and_capped_by_caller() {
        let e =
            Error::UnknownOption { given: "--colour".into(), suggestions: vec!["--color".into()] };
        assert_eq!(e.to_string(), "unexpected argument '--colour', did you mean '--color'?");
        assert_eq!(e.kind(), "unknown-option");
    }

    #[test]
    fn two_suggestions_use_or_join() {
        let e = Error::UnknownOption {
            given: "-z".into(),
            suggestions: vec!["-v".into(), "-q".into()],
        };
        assert!(e.to_string().ends_with("did you mean one of: -v or -q?"), "{e}");
    }

    #[test]
    fn help_and_version_exit_zero_everything_else_two() {
        assert_eq!(Error::Help(String::new()).exit_code(), 0);
        assert_eq!(Error::Version(String::new()).exit_code(), 0);
        assert_eq!(Error::MissingValue { option: "x".into() }.exit_code(), 2);
    }
}
