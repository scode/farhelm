# Report: five spec clarifications and two discards from the 2026-10-22 triage

## What this was about

Seven review findings you decided on 2026-10-22, all specification text or review-queue bookkeeping, with no product
code change:

- A user's own tmux server or ssh ControlMaster that a session's agent starts first (agents run without tmux's `TMUX`
  variable, so a plain `tmux` there reaches or starts the user's own server) is ended by Stop, Restart or Delete, along
  with windows the user later opened in it from outside Farhelm. You said this is intended ("If you're running a [tmux]
  from within that Farhelm-managed [tmux], you're on your own"), and asked for it to be written down.
- The spec justified a terminal tab owning the services its startup files start first by saying the agent's launch
  usually starts them already. For tmux that is false: the launch runs the startup files while `TMUX` still names
  Farhelm's private server, so a `TMUX`-guarded tmux autostart fires in the first tab instead, and with `exec tmux` a
  detach ends that tab and the server. You kept the rule and asked for the premise to be corrected.
- A Goose sub-agent or nested Goose run, and a second Pi started under the agent that picks up Farhelm's Pi integration,
  can replace which conversation Resume reopens. You chose to state both as accepted limits.
- A checkout root (the folder managed checkouts are created in) that more than one host can reach (a network mount, say)
  lets one host trash and later delete a checkout another host's session is working in. You chose to declare it
  unsupported, with no detection.
- Two findings you agreed to drop: an open session launcher that is handed a different source keeping the conversation
  picked for the old one (no shipped way to hand it a new source while it is open was found), and Cancel on the
  launcher's loading screen discarding an accepted launch (the loading screen and its Cancel are gone before a launch
  can be accepted).

What landed: each of the five clarifications is written into SPEC.md where the triage entry asked (the two tmux ones
also in SPEC_impl.md and in the TODO.md "Doc todo" item about tab-started services, which was widened and stays open for
whoever writes the user docs), and each of the seven feedback files is removed from the review queue with its triage
outcome marked complete.

## Things you should know

- Your principle for the tmux and ssh ControlMaster case said ending the session "(Stop, Restart, Delete, tab close)"
  ends such a program. The spec already says Stop and Restart leave terminal tabs running, so the new text splits the
  two cases: a program the agent started first ends with Stop, Restart, Replace or Delete; one a tab started first ends
  when that tab closes or the session is deleted. The review caught the conflict in my first draft. If you meant Stop to
  end tab-started programs too, that would be a behaviour change and a separate decision.
- That same text carries the existing caveat for hosts without a usable systemd user manager (macOS, or Linux without
  the user manager), where a detached process that hides its environment can survive cleanup, and says plainly that this
  is intended, not a defect, so the finding does not come back.
- The tab correction keeps the general premise for other services (an ssh-agent, an editor daemon), which the triage
  entry did not dispute, and adds tmux as the routine exception, conditioned on no tmux server of the user's running
  yet.
- The Goose and Pi limits are a new paragraph under "Durability and resume", labelled as an accepted exception to the
  promise that Restart resumes exactly the session's own conversation, so the two do not read as a contradiction. The Pi
  sentence says "replace or withdraw", since an early report from a child Pi can also withdraw the Resume target.
- The shared checkout root is a new paragraph right after the rule that Farhelm owns its managed checkouts, and is added
  to the list of unsupported configurations Farhelm need not refuse.

## Open questions and possible follow-ups

- SPEC.md elsewhere still says integration is never switched on through the environment, while SPEC_impl.md says Goose
  is the one harness that still is (which is the cause of the accepted Goose limit). The reviewer flagged the mismatch;
  reconciling it is outside these outcomes.
- TODO.md's "Maybe later" entry about a nested Grok says Pi is unaffected because a Pi started inside a session reports
  nothing. Next to the accepted Pi limit that is now slightly incomplete. I left it, since this plan does not address
  that entry.
- The "Managed checkouts" section, where the checkout root is configured, has no pointer to the new shared-root rule. A
  short cross-reference would help whoever writes user docs for it; not required by the entry.

## PRs

- [#2626](https://github.com/scode/farhelm/pull/2626/changes)
  `docs: state that a session owns a tmux server its agent starts`
- [#2627](https://github.com/scode/farhelm/pull/2627/changes)
  `docs: note that a TMUX-guarded tmux autostart runs in the first tab`
- [#2628](https://github.com/scode/farhelm/pull/2628/changes)
  `docs: accept that a nested Goose run can replace the Resume target`
- [#2629](https://github.com/scode/farhelm/pull/2629/changes)
  `docs: accept that a child Pi can replace the Resume target`
- [#2631](https://github.com/scode/farhelm/pull/2631/changes)
  `docs: declare a checkout root shared between hosts unsupported`
- [#2632](https://github.com/scode/farhelm/pull/2632/changes)
  `chore: drop the finding about a launcher keeping a stale conversation`
- [#2633](https://github.com/scode/farhelm/pull/2633/changes)
  `chore: close the finding about Cancel on the launcher's loading screen`

## Checks

Run now: `dprint check` on every changed file (SPEC.md, SPEC_impl.md, TODO.md, TRIAGE_OUTCOMES.md and the review queue
index), and `python3 scripts/check-agents-md.py` for the removed queue files and the new spec anchor links, on the final
version of the stack. Both clean.

Reused: none.

Skipped: all Rust, browser, desktop and other runtime tests. The changes are specification text and queue bookkeeping
only, which the repository's rules exclude from runtime testing.

## Review gate

Each PR got a fresh-context Opus 5.5 reviewer at high effort, reviewing correctness, design and idiom; the two
bookkeeping-only discards shared one reviewer, who reported on each separately. None found anything blocking. I applied
every finding that fit the agreed scope: the Stop and Restart correction above (the only medium-severity one), the
systemd caveat and Replace in the tmux text, clearer wording of the tab correction in all three places, the separate
paragraphs and the usual "Confirmed" label for the Goose, Pi and shared-folder text, and "replace or withdraw" for Pi.
Not applied, as outside these outcomes: the environment wording mismatch and the TODO.md Pi note above, a
cross-reference from "Managed checkouts", and a nearby sentence about sub-agent reports that could be read as covering
Goose. The two discards had no findings.
