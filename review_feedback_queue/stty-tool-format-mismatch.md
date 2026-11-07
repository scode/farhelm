# Use the same stty implementation for both terminal-settings samples

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The terminal-settings test can fail on a Mac with GNU command-line tools first on PATH even when the host's terminal
settings are correct. Its reference and host samples use different stty implementations, whose saved-settings strings
cannot match.

## Details

Source coordinates from the original records: `crates/terminal-host/tests/host_process.rs:128`. References below use
these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Introduced by 97f7158c5b2811a24daf88439e1fb73a7d9cd655 and unchanged at the reviewed tip. host_process.rs:48 runs
/bin/sh -c 'stty -g; stty -a' through run_to_exit, which calls common::spec (:19-25). common/mod.rs:33-45 supplies the
host child an explicit PATH=/usr/bin:/bin. The reference_settings helper instead runs Command::new("stty") at :128-132
with the test process's inherited PATH. The assertion compares their exact first-line strings (:51-55). No executable
selection makes these commands the same.

Source-established trigger: on macOS, prepend Homebrew coreutils/libexec/gnubin to PATH, as its official formula
documents. The formula installs unprefixed symlinks for all commands (:89-95) and documents that PATH selection
(:108-117): https://raw.githubusercontent.com/Homebrew/homebrew-core/master/Formula/c/coreutils.rb . This makes the
reference GNU stty while the host's fixed PATH selects Apple's stty. Apple's official stty.c selects gprint for -g
(:81-83,109-111), and gfmt.c:57-66 prints the literal gfmt1: prefix plus named fields:
https://raw.githubusercontent.com/apple-oss-distributions/adv_cmds/main/stty/stty.c and
https://raw.githubusercontent.com/apple-oss-distributions/adv_cmds/main/stty/gfmt.c . GNU's display_recoverable,
stty.c:2068-2093, emits only hexadecimal colon-separated numbers (no gfmt1 or field names), including on BSD:
https://raw.githubusercontent.com/coreutils/coreutils/master/src/stty.c . Thus the string comparison necessarily differs
even for identical termios. This conclusion is source inspection, not a runtime reproduction or a claim that any current
worker has this PATH.

The relevant contract remains terminal-host/SPEC.md, 'The child': a new pty's kernel defaults with erase DEL and IUTF8
enabled. The test's own :35-45 comments state that it intends to compare precisely that contract. Different
serialization is not a permitted terminal-setting difference and proves no host regression. A fix can give the reference
command the same explicit child PATH (per-command injection, without mutating the test process environment), or select
one absolute stty executable for both samples.

Suggested bucket: other, because the established consequence is a false failure in maintainer tests, not security or
lost user work. Confidence: confirmed from source for the stated macOS/GNU-PATH configuration. proposed_drop: false.
possible_cover: none known; distinct from prior pty descriptor candidate c6 and all peer cleanup/observer findings. This
is a genuinely new verified test-portability concern. No runtime experiments or checkout edits.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g001/l14`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
