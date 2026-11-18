//! Nested subcommands:
//!
//! ```console
//! $ cargo run --quiet --example gitlike -- branch list --sort date -a
//! $ cargo run --quiet --example gitlike -- commit -aa -T Co-authored-by:x
//! ```

use karg::{Cmd, Matches};

fn main() {
    let cmd = Cmd::new("gitlike", env!("CARGO_PKG_VERSION"))
        .about("A three-level CLI declared in one expression")
        .long_flag("no-pager", "Never pipe output through a pager")
        .subcommand(branch())
        .subcommand(commit())
        .require_subcommand();

    let m = cmd.parse_or_exit();
    dispatch(&m);
}

fn branch() -> Cmd {
    Cmd::new("branch", "0.1.0")
        .about("List, create and delete branches")
        .subcommand(
            Cmd::new("list", "0.1.0")
                .about("Show branch names")
                .long_opt("sort", "Ordering key")
                .value_name("KEY")
                .default("refname")
                .flag("all", 'a', "Include remote-tracking branches"),
        )
        .subcommand(
            Cmd::new("delete", "0.1.0")
                .about("Remove a branch")
                .flag("force", 'f', "Delete even if unmerged")
                .arg("name", "Branch to remove"),
        )
}

fn commit() -> Cmd {
    Cmd::new("commit", "0.1.0")
        .about("Record staged changes")
        .flag("all", 'a', "Stage every tracked file")
        .flag("amend", 'A', "Rewrite the previous commit")
        .multi("trailer", 'T', "Append KEY=VALUE to the message")
        .value_name("KEY=VALUE")
        .opt_arg("message", "Message, otherwise $EDITOR opens")
}

fn dispatch(m: &Matches) {
    let Some((name, sub)) = m.subcommand() else {
        return; // `require_subcommand` makes this unreachable.
    };
    match (name, sub.subcommand()) {
        ("branch", Some(("list", inner))) => println!(
            "branch list  sort={}  all={}",
            inner.get_or("sort", "refname"),
            inner.flag("all")
        ),
        ("branch", Some(("delete", inner))) => println!(
            "branch delete {}  force={}",
            inner.positional(0).unwrap_or("?"),
            inner.flag("force")
        ),
        ("branch", None) => println!("branch: pick `list` or `delete`"),
        ("commit", None) => println!(
            "commit  -a x{}  amend={}  trailers={:?}  message={:?}",
            sub.count("all"),
            sub.flag("amend"),
            sub.strings("trailer"),
            sub.positional(0)
        ),
        (other, _) => println!("unhandled: {other}"),
    }
}
