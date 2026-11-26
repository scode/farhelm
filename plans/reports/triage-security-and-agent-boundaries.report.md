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
