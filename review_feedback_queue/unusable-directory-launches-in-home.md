# An unusable session folder can launch work in HOME

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Creating or restarting a session, or opening one of its terminal tabs, can succeed in HOME or `/` when the selected
folder cannot be entered. Farhelm still identifies the session with the selected folder. Subsequent commands could act
on unrelated files; no harmful writes have been demonstrated.

## Details

`crates/farhelm-supervisor/src/service/core.rs:3522–3535` checks that the path is absolute and its metadata describes a
directory. For an unprivileged owner, a final directory with mode `0600`, searchable parents, and no trailing slash can
pass that check and canonicalization while failing `chdir`. Root can bypass this permission trigger.

The reviewed tmux 3.7c [spawn implementation](https://raw.githubusercontent.com/tmux/tmux/3.7c/spawn.c) falls back to
HOME, then `/`, when entering the requested directory fails. Farhelm's launch specification carries no expected cwd
(`launch.rs:137–197`), and its command, shim and scope wrapper do not enforce one
(`launch.rs:626–696,857–1035,1060–1125`; `scope.rs:897–926`). Thus confirmation can observe a running pane without
proving where its command began.

The three traced entry points are initial Create (`service/core.rs:7522–7528,7648–7672,9948–9975`; `tmux.rs:2846–2892`),
Restart (`service/core.rs:3133–3151,11453–11459`), and OpenTab (`service/core.rs:12642,12932`; `tmux.rs:2976`;
`launch.rs:710–723`). OpenTab confirms shell liveness, which the fallback shell can satisfy. Restart's canonical
identity comparison does not establish enterability. Fresh managed preparation has an absolute clone destination and may
fail before agent execution; do not infer that every managed creation silently clones at home. Ready-managed preparation
was examined as a related interface, not reproduced.

Suggested bucket: provisionally **highest**, because the unresolved consequence is execution against unintended work.
The create, tab and core reviewers suggested `high` for the established wrong-directory behavior and potentially
`highest` for damaging follow-on commands; the restart reviewer suggested `highest`. Confidence: **static fallback and
missing enforcement established; end-to-end permission regression unrun**. Shell startup or an individual agent may
independently change, reject or correct cwd. No destructive command, data loss or remote authority escape was observed.

Possible cover: SPEC.md:2792–2799 explicitly requires unusable tab paths to fail clearly. Its accepted symlink
resolution is a different trigger. The ledger's three `tmux-cwd-format-expanded-*` items and
`restart-cwd-lossy-non-utf8.md` share the HOME consequence but fix format expansion and lossy encoding, not missing
search permission. The deliberate path-replacement acceptance does not establish intent to execute in HOME after a
mistaken permission change. Planned create dispatch concerns scheduling. No exact BUGS or queue match was found.

Proposed drop: none. Verify with an isolated unprivileged process and inspect actual child cwd. Enforcement must prevent
the command or agent from starting if entry into the intended directory fails; another preflight alone leaves a race.

Provenance: `supervisor-create-1/f3` (`turn3.result.md`), `supervisor-restart-1/f4` (`turn3.result.md`),
`supervisor-terminal-control-1/f2` and `supervisor-core-contracts-1/f3` (both `turn2.result.md`). One underlying
launch-directory enforcement gap; no new runtime evidence was generated for this consolidation.
