## What this was about

Eight review findings called for changes to introductory claims, old release notes, and three specification promises.
The maintainer chose five specification changes, with no product-code or website change.

The specification now treats the README introduction, docs Overview and their artwork as a short hook for the main,
first-class path, with agent and host qualifications in the guides. The findings concerned broad promises about waiting
alerts, automatic host installation and conversation recovery after reboot. Only Claude and Codex report Waiting;
automatic host setup requires a usable systemd user manager and a supported architecture; conversation recovery needs
Resume support and a captured conversation. Those qualifications stay in the guides instead of expanding the hook.
Published release notes and screenshots remain the historical record, corrected only for catastrophically major mistakes
with huge user impact. Both principles preserve the maintainer's wording.

## Things you should know

Three accepted limits are now explicit:

- A template catalog whose agent listing exceeds one reply frame, about 8 MiB or roughly 130 templates near the
  per-field cap, is outside the supported scale. An agent running `farhelm agent templates` inside a session may receive
  a failure instead of the catalog until templates are removed. The GUI's Templates dialog does not use that frame limit
  and can still list and remove templates.
- A session's hover summary may clip long values or whole rows in a short window. The session view provides the full
  details; the spec no longer promises a full directory in that preview.
- After an interrupted checkout archive, startup recovery or a Delete retry can clear Claude's trust record for a
  different folder later placed at the old path. Claude may ask about trust again.

These are documentation corrections and accepted support boundaries. No implementation, introduction, artwork or
published release artifact changed. All eight outcome records name their change, bookmark and PR; their feedback files
and index entries were removed. There were no size gates, discards, blocks or scope departures. The five specification
diffs are 7, 4, 4, 5 and 7 lines respectively, excluding outcome and queue bookkeeping.

One commit-path mistake initially left the first draft's feedback cleanup uncommitted. It was corrected in that same
draft before proceeding; the final change contains the originally reviewed removals. The working log follows the
`farhelm-plan-<slug>-log.md` rule beside the checkouts.

## Open questions and possible follow-ups

None required by this plan. The large-catalog listing failure and reused-path trust behavior remain as accepted above.

## The PRs

- [#2413 — define the introduction's scope](https://github.com/scode/farhelm/pull/2413/changes), covering three
  findings.
- [#2414 — preserve published release notes](https://github.com/scode/farhelm/pull/2414/changes), covering two findings.
- [#2416 — bound supported template catalog scale](https://github.com/scode/farhelm/pull/2416/changes).
- [#2418 — allow hover summaries to clip](https://github.com/scode/farhelm/pull/2418/changes).
- [#2420 — clarify archive recovery's trust cleanup](https://github.com/scode/farhelm/pull/2420/changes).

All five are open drafts in one linear stack, in that order.

## Checks run, reused and skipped

Run: changed-file `dprint check` for each PR and the final combined
`dprint check SPEC.md SPEC_impl.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`, all passed.
`python3 scripts/check-agents-md.py` passed after each feedback removal and at the final combined check, with zero
errors or warnings. A separate documentation pass inspected every touched file.

Reused: each independent review covers its final specification text and feedback removal. Later amendments only added
the required PR links to outcome records. The first draft's cleanup correction restored the removals in its original
reviewed diff. Main changes from base `e478911f` through `2ea34288` were read in full and contain only other plans'
claims and delivery reports; they require no rebase or additional validation.

Skipped: Rust, JavaScript, browser, desktop, installer and provisioning tests and builds. These ordinary Markdown edits
change no executable example, generated artifact or runtime behavior. No CI was dispatched, and no runtime result from
an earlier plan is claimed as evidence here.

## Review gate's outcome

Each PR passed exactly one fresh-context gpt-6.1-sol review at high effort, with the full goal and relevant outcome
entries. All five returned no findings. Their commit messages passed separate cold reads. The report passed its
independent cold read after adding product context for the introduction claims and template listing.
