# Codex's option spelling of YOLO escapes the sensitive-host check

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

Starting Codex with `-a never -s danger-full-access` (Codex's spelled-out form of its no-approvals, no-sandbox mode) on
a host marked sensitive skips Farhelm's YOLO confirmation, and the sidebar shows no YOLO badge.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F57 / SEC-CODEX-YOLO-OPTIONS`, tagged **definite**. Anchor and title: `crates/farhelm-proto/src/yolo.rs:116` —
the YOLO classifier misses Codex's documented option form for the same mode.

Farhelm requires an explicit confirmation before starting a YOLO launch (one that turns off the agent's approval
prompts) on a host the user marked sensitive, and SPEC.md says the helm enforces this for any command line carrying a
flag its vendor documents as skipping approvals. For raw commands and profiles, the helm decides "is this YOLO?" with a
classifier in `crates/farhelm-proto/src/yolo.rs` (`argv_is_yolo`, called from
`crates/farhelm-helm/src/yolo_guard.rs:56-58`); the sidebar's YOLO badge uses a sibling function in the same file
(`invocation_marker`, called from `crates/farhelm-ui/src/list/row.rs:579`).

The classifier works from two tables. One lists stand-alone flags per program; for Codex that is
`--dangerously-bypass-approvals-and-sandbox` and `--yolo` (YOLO), plus `--full-auto`, which SPEC.md explicitly says does
not count because it stays sandboxed. The other table (`YOLO_OPTION_VALUES`, lines 116-125) lists YOLO modes spelled as
an option with a value: OMP's `--approval-mode yolo`, and Claude's `--permission-mode bypassPermissions`, which is there
precisely because Claude documents it as the same mode as `--dangerously-skip-permissions`. Codex has no entry in that
second table. Yet, per the reviewer, Codex documents `--ask-for-approval never --sandbox danger-full-access` (short form
`-a never -s danger-full-access`, or the `=` spellings) as the same behavior, with
`--dangerously-bypass-approvals-and-sandbox` being shorthand for it. The badge parser already knows that
`-a`/`--ask-for-approval` and `-s`/`--sandbox` take values (lines 236-251), so it reads past them, but nothing treats
their YOLO values as YOLO.

As a result, `codex -a never -s danger-full-access`, whether in a profile, a New-session command, a raw
`farhelm agent create`, or a clone/replace seeded from such a session, is classified as not YOLO. It starts on a
sensitive host with no confirmation and without `--allow-yolo-on-sensitive-host`, and the sidebar shows no YOLO badge.
This is separate from F1: F1 is the classifier failing to find the program at all behind an `env NAME=value` prefix;
this is Codex's rule set being incomplete even when the program is found. The confirmation is the only control between a
permission-skipping agent and a host the user marked sensitive, Codex is a first-class harness, and the code already
accepts that option-value spellings are in scope for Claude and OMP.

The suggested fix is to treat a Codex command as YOLO when its switches carry `--sandbox danger-full-access` (or the
`-s`/`=` spellings) together with `--ask-for-approval never` (or `-a never`), and also when the same settings arrive
through `-c`/`--config` (`approval_policy="never"` with `sandbox_mode="danger-full-access"`). The maintainer should also
decide, and record in SPEC.md next to the `--full-auto` sentence, whether `-a never` alone (which keeps Codex's default
workspace-write sandbox) counts. Add classifier tests for each spelling and a guard test that refuses such a create on a
sensitive host.

Restater note: the claim that Codex documents `-a never -s danger-full-access` as equivalent to
`--dangerously-bypass-approvals-and-sandbox` is vendor documentation outside the checkout and was not verified here;
nothing in the repository mentions `danger-full-access`. The code-side claim (no Codex option-value entry, so that
spelling classifies as not YOLO) is confirmed.
