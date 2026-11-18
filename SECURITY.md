# Security policy

## What counts as a vulnerability here

`karg` parses hostile input for a living, so these are all in scope:

- Unbounded memory growth or a panic reachable from `parse()`/`parse_std()` on
  any input shorter than a few megabytes.
- Any path that turns user-controlled argument text into a shell command,
  format string, or path traversal inside this crate.
- A `--help` rendering that injects escape sequences beyond the plain text the
  declaration asked for.
- Anything requiring `unsafe` — there is none, and `#![forbid(unsafe_code)]` is
  compile-enforced, so an `unsafe` block appearing in a release is itself a bug.

Not in scope: what the calling application does with the strings it receives,
and non-UTF-8 `argv` handling (documented behaviour: `Error::InvalidUtf8`).

## Reporting

Open a **private** security advisory on the repository rather than a public
issue:
[Report a vulnerability](https://github.com/fj-8/karg/security/advisories/new).

If you would rather not use GitHub, mail the maintainer address shown on the
profile. Expect an acknowledgement within a week; this is a side project, so the
fix may take longer than a company SLA, but the disclosure timeline is yours to
set and will be respected.

## Supported versions

| Version | Supported |
| --- | --- |
| 0.3.x | ✅ |
| < 0.3 | ❌ — no pre-release versions were published |

## Patch and disclosure

A fix lands on `main` first, is tagged as a patch release, and the advisory is
coordinated with you before the release notes mention the class of bug. If the
issue needs no private handling, a normal issue and PR are welcome instead.
