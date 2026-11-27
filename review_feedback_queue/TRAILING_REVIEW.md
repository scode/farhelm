# Trailing code review checkpoint

Last reviewed commit: `b2d540a8dacfb94140e5eb0927d78e447ce46111`

This is the exclusive starting point for the next trailing code review of `main`. Advance it only after completing the
procedure in `review_feedback_queue/.agents/trailing-review.md`, together with any resulting queue updates. A proposed
checkpoint reaches `main` only when its review-result PR is merged.

This file holds only the checkpoint and the latest run's completion record. Earlier runs' records are in
`review_feedback_queue/TRAILING_REVIEW_HISTORY.md`.

## Review through b2d540a8

The range starts after `dbb153ec94149a598a747af8ab306a3df0514641` and includes
`b2d540a8dacfb94140e5eb0927d78e447ce46111`. Of the 272 commits inventoried, 49 have in-scope patches with 20,412
additions and 5,916 deletions across individual commits, touching 180 unique paths: Settings saves and closing while
work is out (the credential editor, the Tensorlake SSH key and key forms, the recipe editor's leave and Delete
questions, custom agent draft names and the footer binding); the terminal tab close slot and Cmd+T/Cmd+W; Delete with
open tabs and the marked menu's focus; copying a terminal file's contents, the too-large hint and the helm client's
small-file read; Paste via file upload; agents asking for a Tailscale host and the hosts table's Tailscale column;
reliable ntfy waiting alerts, `farhelm agent notify` and asking agents to notify when blocked; Claude's reported
transcript, Restart starting Claude fresh, and Codex fork and app-server conversations; ending a failed restart's host
at startup and the tab sweep's backoff; unchanged session rows, kept change hints, the 30-second host poll and
once-a-minute activity stamps; a helm Replace refusing a changed destination and the removal of the legacy
sandbox-sessions route and dead Replace checkout code; Stop's refusal wording and the agent-Delete cleanup notices; CLI
option parsing and the installer's forwarder; the brick-test harness and the release-notes screenshot script; the
real-agent drift tests (base turns, Codex's process tree, Guarded footers, the private config mirror and the
conversation rotation category); and `SPEC.md` and `SPEC_impl.md`. The other 223 commits contain only excluded material:
plans and their reports, review queue and triage bookkeeping, TODO records, changelog fragments, agent instructions,
prose documentation (including the docs website and `docs/` pages), the published release notes and release procedure
prose. Excluded paths inside in-scope commits were read only as context. `docs/gpui/screenshot-scenes.json` and
`docs/docs-shots/scenario.json5` were reviewed as configuration of the screenshot tooling.

Each in-scope commit was reviewed as its own patch, one snapshot per file, with three lines of context: 40,116 mandatory
lines (2,066 KiB) in 355 snapshots, against an extract of the pinned tip for context. Claude Code `claude -p`
(`claude-opus-5-5` at high reasoning effort) reviewed 38 functional groups over 123 turns: an initial, synthesis and
discovery turn per group, further discovery passes for the five groups whose discovery kept finding something new (g01,
g02, g03, g20, g34; g02 also reopened once for cross-group input from g01 and g03), and one reopening continuation each
for the two groups that received a disagreement or lead from the independent drop check (g10, g18). Those two turns ran
outside the scheduler, which cannot reopen a completed identity; they used the same sessions, launch scripts and audits.
Delivery audits matched every mandatory line in every group with no rejected, unfinished or unrecognized records. Every
group ended with a discovery pass that found nothing new, and the run ended quiescent with no open leads.

The reviewers logged 43 findings and 236 set-aside candidates; none was withdrawn, and one candidate was revised into a
finding after the drop check disagreed with its set-aside. Consolidation produced 38 verified defect groups, 2 groups
unverified after investigation and 225 excluded candidate groups. Cross-group duplicates were folded into their items,
and findings from different groups with one cause were merged (the drift runner's per-case budget; three routes to a
waiting close showing a page without its reason). Four fresh-context Claude consultations judged a 12-entry drop
register. They agreed with the plan cover for the Agent is blocked help text
(`blocked-notify-attention-and-debounce.md`, not landed), with two route-not-established filter drops, with two
rare-glitch filter drops, with the spec acceptance of Claude's fresh restart under a wrapper that remaps its home, and
with three possibly-`highest` set-asides (the hidden tab close button, bulk Delete following the switch, and copied
escape bytes, the first only down to a one-frame window). They agreed with the Cmd+W double-tap drop only for its
behavior, leaving the test's overstated claim queued. They disagreed with one rare-glitch filter drop (a second Close
inheriting the first one's refusal), which is recorded, and with the set-aside of a Delete that ends a tab opened after
the drawn frame, which its reviewer then recorded with the consultant's lead folded in. A lead about failed create
reservations went back to its reviewer and established no route. A lead about the not-landed plan's own Settings strings
concerns plan text and was not recorded. The queue adds 34 items (3 `highest`, 5 `high`, 26 `other`) and extends none.
The review inventory, delivery audits, findings logs, disposition log, drop register and verdicts are retained under run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`.
