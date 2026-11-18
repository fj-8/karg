//! `--help` rendering: usage line, aligned option table, word wrapping.

use crate::{ArgSpec, Cmd};

const TERM_WIDTH: usize = 100;
const INDENT: usize = 2;
const GAP: usize = 2;

/// Full help text for `cmd`, addressed by the invocation `path`.
pub(crate) fn render_help(cmd: &Cmd, path: &str) -> String {
    let mut out = String::new();

    if !cmd.about.is_empty() {
        for line in wrap(&cmd.about, TERM_WIDTH) {
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str("Usage: ");
    out.push_str(path);
    if !cmd.opts.is_empty() {
        out.push_str(" [OPTIONS]");
    }
    for spec in &cmd.positionals {
        out.push(' ');
        out.push_str(&positional_slot(spec));
    }
    if !cmd.subs.is_empty() {
        out.push_str(" [COMMAND]");
    }
    out.push('\n');

    if !cmd.subs.is_empty() {
        let rows: Vec<(String, String)> =
            cmd.subs.iter().map(|s| (s.name.clone(), s.about.clone())).collect();
        out.push('\n');
        out.push_str("Commands:\n");
        write_table(&mut out, &rows, 0);
    }

    let mut rows: Vec<(String, String)> =
        cmd.opts.iter().filter(|s| !s.hidden).map(|s| (invocation(s), description(s))).collect();
    rows.push((String::from("-h, --help"), String::from("Print this help")));
    rows.push((String::from("-V, --version"), String::from("Print version")));

    out.push('\n');
    out.push_str("Options:\n");
    write_table(&mut out, &rows, 12);

    if !cmd.after_help.is_empty() {
        out.push('\n');
        for line in cmd.after_help.lines() {
            let indent = line.len() - line.trim_start().len();
            let body = line.trim_start();
            if body.is_empty() {
                out.push('\n');
                continue;
            }
            let pad = " ".repeat(indent);
            for (i, wrapped) in
                wrap(body, TERM_WIDTH.saturating_sub(indent).max(24)).into_iter().enumerate()
            {
                if i == 0 {
                    out.push_str(&pad);
                } else {
                    out.push_str(&" ".repeat(indent));
                }
                out.push_str(&wrapped);
                out.push('\n');
            }
        }
    }

    out
}

/// Left column of the table: `-j, --jobs <N>`.
fn invocation(spec: &ArgSpec) -> String {
    let short = spec.short.map_or_else(String::new, |c| format!("-{c}, "));
    if spec.takes_value {
        format!("{short}--{} <{}>", spec.long, spec.value_name)
    } else {
        format!("{short}--{}", spec.long)
    }
}

/// Right column: help text followed by `[default: x]`-style markers.
fn description(spec: &ArgSpec) -> String {
    let mut markers = Vec::new();
    if spec.required {
        markers.push("[required]".to_string());
    }
    if let Some(default) = &spec.default {
        markers.push(format!("[default: {default}]"));
    }
    if spec.multiple {
        markers.push("[repeatable]".to_string());
    }
    let tail = markers.join(" ");
    if spec.help.is_empty() {
        tail
    } else if tail.is_empty() {
        spec.help.clone()
    } else {
        format!("{} {tail}", spec.help)
    }
}

fn positional_slot(spec: &ArgSpec) -> String {
    let inner = format!("<{}>", spec.value_name);
    if spec.variadic {
        format!("{inner}...")
    } else if spec.required {
        inner
    } else {
        format!("[{}]", spec.value_name)
    }
}

/// Emit `left`/`right` rows aligned on the widest left column, which is never
/// narrower than `min_left`.
fn write_table(out: &mut String, rows: &[(String, String)], min_left: usize) {
    let left_width = rows.iter().map(|(left, _)| char_len(left)).max().unwrap_or(0).max(min_left);
    let right_width = TERM_WIDTH.saturating_sub(INDENT + left_width + GAP).max(24);

    for (left, right) in rows {
        out.push_str(&" ".repeat(INDENT));
        out.push_str(left);
        if right.is_empty() {
            out.push('\n');
            continue;
        }
        out.push_str(&" ".repeat(left_width - char_len(left) + GAP));
        let lines = wrap(right, right_width);
        out.push_str(lines.first().map(String::as_str).unwrap_or(""));
        out.push('\n');
        for extra in &lines[1..] {
            out.push_str(&" ".repeat(INDENT + left_width + GAP));
            out.push_str(extra.trim_start());
            out.push('\n');
        }
    }
}

/// Greedy word wrap; words longer than `width` stay on their own line.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let extra = if current.is_empty() { 0 } else { 1 };
        if !current.is_empty() && char_len(&current) + extra + char_len(word) > width {
            lines.push(std::mem::take(&mut current));
        } else if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cmd;

    fn fixture() -> Cmd {
        Cmd::new("crop", "1.0.0")
            .about("Crop an image")
            .flag("verbose", 'v', "Print what is happening")
            .opt("format", 'f', "Output format")
            .default("png")
            .multi("tag", 't', "Metadata tag")
            .long_opt("trace", "Internal tracing")
            .hidden()
            .arg("source", "Image to read")
            .opt_arg("target", "Image to write")
            .rest("extra", "Remaining args")
            .after_help("Examples:\n  crop a.png b.png -f webp")
    }

    #[test]
    fn usage_line_lists_options_and_positional_slots() {
        let help = fixture().help_text();
        assert!(help.starts_with("Crop an image\n\nUsage: crop"), "{help}");
        assert!(help.contains("[OPTIONS] <SOURCE> [TARGET] <EXTRA>..."), "{help}");
    }

    #[test]
    fn commands_section_lists_subcommands() {
        let cmd = Cmd::new("ctl", "1")
            .subcommand(Cmd::new("run", "1").about("start the job"))
            .subcommand(Cmd::new("stop", "1").about("stop it"));
        let help = cmd.help_text();
        assert!(help.contains("[COMMAND]"), "{help}");
        assert!(help.contains("  run   start the job"), "{help}");
        assert!(help.contains("  stop  stop it"), "{help}");
    }

    #[test]
    fn hidden_options_are_omitted_and_builtins_present() {
        let help = fixture().help_text();
        assert!(!help.contains("--trace"), "hidden option leaked:\n{help}");
        assert!(help.contains("-h, --help"));
        assert!(help.contains("-V, --version"));
        assert!(help.contains("-t, --tag <TAG>"));
    }

    #[test]
    fn trailers_show_default_repeatable_and_required() {
        let help = fixture().help_text();
        assert!(help.contains("[default: png]"), "{help}");
        assert!(help.contains("[repeatable]"), "{help}");
        let cmd = Cmd::new("r", "1").opt("out", 'o', "Target").required();
        assert!(cmd.help_text().contains("[required]"));
    }

    #[test]
    fn every_line_respects_the_width_cap() {
        let cmd = Cmd::new("w", "1").long_opt(
            "strategy",
            "Choose how aggressively the indexer merges runs which matters when \
             documents are large and rows are sparse and the description is long",
        );
        let help = cmd.help_text();
        for line in help.lines() {
            assert!(
                line.chars().count() <= TERM_WIDTH,
                "too long ({}): {line}",
                line.chars().count()
            );
        }
        assert!(
            help.lines().any(|l| l.starts_with(' ') && l.contains("merges")),
            "expected a wrapped continuation line in:\n{help}"
        );
    }

    #[test]
    fn after_help_block_is_appended_last() {
        let help = fixture().help_text();
        assert!(help.contains("Examples:\n  crop a.png b.png -f webp"), "{help}");
        assert!(help.trim_end().ends_with("crop a.png b.png -f webp"), "{help}");
    }

    #[test]
    fn wrap_splits_on_words_only() {
        assert_eq!(wrap("aaa bbb ccc", 9), vec!["aaa bbb", "ccc"]);
        assert_eq!(wrap("single", 3), vec!["single"]);
        assert_eq!(wrap("", 10), vec![""]);
    }

    #[test]
    fn short_only_option_renders_without_long_name() {
        let cmd = Cmd::new("s", "1").flag("quiet", 'q', "Silence output");
        assert!(cmd.help_text().contains("-q, --quiet"), "{:?}", cmd.help_text());
    }
}
