# Agent request boundaries and host identity pinning

## What this was about

An agent can ask Farhelm to create a session or an environment within the permissions the user gave it. An approved
Podman environment could proceed after the chosen machine or account changed, and a standing grant could let a request
proceed while the session that made it was being deleted. An agent's checkout could also add a repository to the
launcher on other hosts. The triage decisions were to bind the environment approval to its chosen host, refuse requests
from a deleting session, and reserve new repository suggestions for user launches. The plan also escapes the requesting
session in Create logs and adds a test for the existing connection check on an approved command launch.

## Things you should know

- An approval for an agent's Podman environment belongs to the chosen host connection and the recorded Podman machine
  and account. If those change while approval is pending or before Farhelm registers the new environment, the request
  must be made again. The fix uses the existing place claim and registration transaction.
- Farhelm refuses requests from a session while its deletion is in progress, including requests covered by a standing
  command, session or native-link grant. At the final dispatch boundary, an approved command launch cannot move to a
  replacement host connection. The create log renders the originating session reference safely on one line.
- New shared Managed checkout suggestions come from user launches on every host. Existing history is retained. Farhelm
  still recognizes an agent's checkout as temporary so its folder does not enter ordinary folder suggestions.

## Open questions and possible follow-ups

The second full desktop smoke had a separate Rename failure. The step chose the first seeded session in its menu and the
retained supervisor record showed the second seeded session received the new title. Rename passed on the first run. The
cause remains open, and this plan leaves that UI flow to separate investigation.

## Pull requests

- Bind agent environment approval to the chosen Podman host. [#5030](https://github.com/scode/farhelm/pull/5030/changes)
- Refuse requests from deleting sessions, verify the last Create connection check, and escape the requesting session in
  logs. [#5032](https://github.com/scode/farhelm/pull/5032/changes)
- Limit new repository suggestions to user-initiated checkouts.
  [#5035](https://github.com/scode/farhelm/pull/5035/changes)

## Checks

- The 55-test Helm selection passed at `12d5bd7867eb7` (record `744613b5-cf58-4028-91ca-aad493226870`). It covered a
  change to the recorded Podman account before and after approval, and a reconnect with the recorded machine and account
  intact. The pushed first PR has the same source and tests.
- The 38-test Helm selection passed at `e932c3a98236` (record `46c7313b-3669-4812-b345-1b2212af1c3f`). It checked
  refusal during session deletion despite standing grants, the refusal to dispatch an approved Create after the target
  reconnects, and the escaped session reference in the log. The amended timeout diagnostic ran in the 53-test tip
  selection at `a10957f82d59` (record `c1eb3267-d8e7-4814-bdfd-7c548a715034`). All 53 passed, including the Managed
  checkout tests for both host ownership modes and the approval request regressions. The final push added the outcome
  records and queue cleanup.
- Rust formatting, Helm Clippy, the test-sleep checker, changelog format and targeted Markdown checks passed. The
  archive-wide checks reported existing formatting and an existing heading citation warning.
- The staged desktop ran against `e61e021027ea`. The first full run recorded a child created by Allow, then failed while
  reading the wrong focused terminal (record `5ca7aef2-1c47-4449-9596-4738019a6ab6`). The unmodified Allow step passed
  in isolation (record `eeecfca0-c1b8-4853-9d3d-d689d3b2362e`). The warm full rerun passed all four approval scenarios
  and failed only Rename (record `d24451f5-b6cb-4a99-b32d-8ff74acb67d6`). Each full run had 47 passes, one failure and
  the expected Linux clipboard skip.
- The place tests used owned stand-in supervisors and provider dispatch counters. Real provisioning and graphical golden
  comparisons were skipped because the new behavior is at the Helm approval, persistence and dispatch boundaries.
- Main through `ffc1c04bf16c` moved Settings presentation, restart hooks, Antigravity argument ordering and release test
  tooling. The stack relies on separate request and history boundaries, so its source evidence still applies and the
  stack kept its tested base for delivery.

## Review

Independent Sol source review passed for all three PRs. The first review led to an independent reconnect witness and
clearer changelog wording. The second led to a clearer timeout diagnostic in the test. The third approved the repository
projection change and flagged the queue archival requirement, which the final draft includes.

### Landing

#### What landed with it

This plan landed first in its round; the host menu and terminal triage (`triage-gui-hosts-menus-and-terminal`) landed
right after it and shares only the triage records with it. Since this plan's PRs were based, a long run of work reached
main (the Settings triage, the hook warning and Antigravity Restart fixes, the host provisioning fixes, the Welcome and
What's new changes and more). The rebase met only conflicts in the shared triage records, kept both ways. The landing's
independent review found no interaction: nothing that landed adds a caller of what this plan changes or contradicts its
spec edits, and no schema or protocol version is involved.

On the security side the review confirmed what the plan set out to do: an approved agent container launch is bound to
the container host's recorded machine, account and connection, checked before the approval card, after Allow, and again
when the environment is registered; every agent request that acts goes through the approval step, which now refuses a
session that is being deleted before any standing permission applies; and only launches you start add repository
suggestions.

#### Fixes made while landing

- **A doubled blank line in the review feedback queue's index,** left where the plan removed the only earlier entry
  under its highest priority heading, which `dprint check` refuses. Fixed inside
  [#5030](https://github.com/scode/farhelm/pull/5030/changes).

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on both plans' PRs stacked in queue order on main as it stood at the start of the landing (`1e344451`), with the shared
triage records' conflicts kept both ways; every other change is exactly what the executors pushed. The one landing fix
since, inside #5030, removes a blank line from the review feedback queue's index, so these runs still apply.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `3ab311fb-56ce-4f7e-a781-048ff52d03ee`): all 8755 passed, with no runtime `SKIPPED` lines. It covers both plans' helm
  and GUI tests.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `da502306-29fe-48a5-af71-405b24619819`): of its 49 steps, 48 passed, including Rename, the spawn approval steps, the
  host menus, Stop and the templates, and one (paste) was skipped by design on Linux.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `5ec3eec6-72fa-4964-9cd6-17b84713fc75`): passed; neither plan changes a reference picture.
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: after the fix it fails only on
  over-long lines in the triage outcomes archive, main's earlier ones (#4978) and the security triage plan's three new
  entries. No PR fixes them yet; a formatting-only one would, and I recommend it, as earlier rounds' notes did.

Reused across the fix: every run above except `dprint check`, which ran again, since the fix changes only whitespace in
the queue's index. The executors' runs were on older bases, and everything they covered ran again above.

Skipped: the desktop app's outer shell tests, since nothing in that crate changed; a Mac run; real Podman and Tensorlake
hosts, which no automated check here reaches; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing is untrue, but the report's open question has more evidence now: the smoke's Rename step, which once gave its
  new title to the wrong seeded session in the executor's second full run, passed in the landing's smoke run, as did the
  spawn approval steps (Allow, the session-wide grant and Deny) that failed in the executor's first full run. One
  failure in three runs is not yet an established flake, so I recorded none; if Rename fails that way again, it is worth
  a FLAKES.md entry.

#### Things you should know that the report does not say

- **The plan's three new triage archive entries have lines longer than the formatter allows.** I left them: rewrapping
  them inside the plan made the next plan's archive entries conflict, and main's archive already had such lines. One
  formatting-only PR of the archive would fix all of them.
- **Smaller points the review noticed.** SPEC_impl.md does not describe the new check when the environment is registered
  or the refusal shown before the approval card; the record that ties an approval to its container host uses a type that
  only the Podman provider has, which the environment module's map of provider decisions does not list (shared code
  still compares no providers, so this is a documentation gap, not a breach of AGENTS.md's provider rule); one test has
  a hard-coded five-second timeout and a wait without a limit; one check in the repository suggestion test cannot fail;
  and the suggestion changelog draft should say suggestions now learn "only" from launches you start. None needs a
  follow-up on its own.
