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

### Landing

Landed as #2413, #2414, #2416, #2418 and #2420, merged bottom-up, with the same content the report describes, rebased
onto this round's main (see the end of these notes). It was the twelfth of sixteen plans in this landing round, after
the eleven listed in the earlier plans' notes, most recently repository-discovery-git-config.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the eleven plans earlier in this round, and documentation
and plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. This
plan changes only SPEC.md, SPEC_impl.md and the review bookkeeping. Several other plans in the round also edit SPEC.md
and SPEC_impl.md, all in different sections, and none of their amendments contradicts these five. The closest is
reachable-at-small-sizes' new paragraph on approval requests in very short windows; it covers a different surface from
this plan's change about the session hover summary, which may now clip in a short window. launch-sequences, later in
this round, edits the template parts of SPEC.md and SPEC_impl.md, but not the template catalog's size limit this plan
adds to SPEC_impl.md. The stricter specification-pointer check from tooling-checks-oct19, later in this round, passes on
these edits. The only conflicts were in the review feedback queue's index, where each plan removes its own entries; all
removals are kept.

#### Fixes made during the landing

None.

#### Checks

Every plan in this round was executed on a main from before round 107's session-identity change, so none had been tested
against it. That is why this round ran the whole Rust workspace and the full browser suite rather than a selection. All
of the following ran now, on all sixteen plans of this round stacked together on main, including the fixes made while
landing plans in this round:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and its `--self-test`,
  `python3 releasing/notes-review-page.py --self-test`, `scripts/check-desktop-assets.sh`, and the website build
  (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `40f2b98b-b160-4239-a653-44cd0bcdcf96`: 3948 passed.
  `cargo nextest run -p farhelm-ui --features desktop`, run `d288b68b-249f-4f21-9fe0-d4d6d1c4a738`: 717 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `f82f83f8-67c9-4e07-99ed-467c9830dc9f`: 317 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, run
  `81d76a96-912f-47ed-8e1f-57423edbeb32` (kept): 2183 passed and 14 skipped. Of the skips, 9 are the WebKit clipboard
  cases, because Playwright cannot grant WebKit clipboard permissions; 4 run only against a real, signed-in agent; and 1
  is a pixel capture that runs only on Chromium. One test failed: on WebKit, "model-only number key opens New with a
  temporary composition explanation" in `templates.spec.ts` timed out at its first step, waiting 60 seconds for the
  header's Templates button to settle before clicking it. No plan in this round changes that button or that test, and
  the file's other WebKit tests that open the same dialog passed.
  `npx playwright test --project=webkit-templates -g 'model-only number key opens New with a temporary composition explanation' --repeat-each=20`
  then passed all 20 (run `6c382916-77a1-4ecb-b4ee-a1dae8cac848`). The cause is probably machine load during the full
  run, not something this round broke, but that is not confirmed; on one observation it is not recorded as a flake.
- Skipped: the native Mac app (not runnable here) and the installer (unchanged).

This plan changes no code, so the runtime suites above only confirm the combined stack; the documentation checks are the
ones that inspect it. The report was written on a main from before round 107's session-identity change, so its statement
that no rebase was needed no longer describes what landed. Nothing else in the executor's report became untrue at
landing.
