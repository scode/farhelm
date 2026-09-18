# Malformed colon-less getent output is accepted as the login shell

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If the system's user database ever answers a shell lookup with garbage, agent launches fail outright (the fake "shell"
does not exist) instead of transparently recovering through the backup lookup — and nothing in the logs points at the
real cause.

## Details

Source: pre-pr-review-swarm, area supervisor-launch, 2026-09-17. Confidence: possible. The parser defect is verified; no
real-world name-service module is known to emit a colon-less line with exit 0, so the trigger is hypothetical.

When `$SHELL` is unset, the supervisor resolves the login shell in two rungs: `getent passwd <euid>` first, then a
direct `getpwuid_r` lookup. The first-rung parser, `parse_getent_passwd_line`
(`crates/farhelm-supervisor/src/launch.rs:342-345`), does `line.rsplit(':').next()` and accepts any non-empty result —
but for a line with no colon at all, `rsplit` yields the whole line, so `Some("weird")` is returned as a valid shell
path. The rung's own contract says unparseable output "is worth a `warn!` even though the fallback still saves the
caller" (launch.rs:260-262), yet this shape triggers neither the warning (the `None` arm at launch.rs:329-332) nor the
fallback. The bogus string becomes the shell binary in every tmux window command built from it, so launches die
immediately. Existing tests pin the two sane shapes (seven-field line; empty shell field) but not this one.

Suggested fix: return `None` unless the line contains at least one `':'`, so a colon-less line takes the same
warn-and-fall-through path as an empty shell field.
