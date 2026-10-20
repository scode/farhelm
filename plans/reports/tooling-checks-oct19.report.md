# Maintainer tooling checks

## What this was about

Three maintainer-tooling bugs were approved for correction, each within a 100-line gate and with its named self-tests. A
link in a release-notes draft could introduce an HTML attribute or executable URL into the review page. Separately, the
instruction checker allowed a local Claude instruction file that suppresses default repository-rule loading, and
accepted a missing implementation-specification pointer as example text.

## Things you should know

The review page now escapes link attributes and permits only HTTP(S) or relative destinations. Refused or malformed
targets leave their label as inert text. Leading browser-ignored control characters are normalized explicitly, avoiding
dependence on the
[URL-parser behavior added in Python 3.12](https://docs.python.org/3/library/urllib.parse.html#urllib.parse.urlsplit).
The tests ran on Python 3.15; no older interpreter or browser exploitation experiment was run. Hostile draft exposure
and consequential harm remain unestablished. This changes the page generator, not published release notes.

The checker rejects inventoried `CLAUDE.local.md` files at the root or below it, while allowing instructions to mention
that prohibited filename. Its inventory boundary is unchanged: ignored files and files outside the repository are not
checked. Missing uppercase specification names with mixed-case underscore suffixes now fail the pointer check; lowercase
example fragment names remain accepted. No Claude installation or configuration was changed.

The three code/test/documentation diffs are 44, 33 and 26 added/deleted lines, including their required validation
inventory updates and excluding outcome bookkeeping and feedback removal. All meet the gate. All three feedback files
and index entries were removed, and the ledger records their changes and PRs. These are `chore` PRs for maintainer-only
behavior, with no changelog fragments. No outcome was discarded or blocked.

## Open questions and possible follow-ups

None required for these outcomes.

## The PRs

- [#2417 — constrain release review links](https://github.com/scode/farhelm/pull/2417/changes), based on main.
- [#2421 — reject local Claude instruction files](https://github.com/scode/farhelm/pull/2421/changes), based on #2417.
- [#2422 — catch mixed-case specification pointer typos](https://github.com/scode/farhelm/pull/2422/changes), based on
  #2421.

## Checks run, reused and skipped

The following self-tests ran through `python3 scripts/record-test-run.py`, development mode, one self-test process,
serial fixtures, and `--tmux none`, on Linux with Python 3.15:

- `python3 -B releasing/notes-review-page.py --self-test`: passed in `4fb93ff3-bdc4-49ef-b05e-98f1e5a99780`, then passed
  with the explicit control-prefix normalization and its added regression in `e04485d1-893f-487f-84a7-ad18b5cf43a0`.
  Both included the existing Node feedback-copy and revision-reload cases; neither reported a runtime skip.
- `python3 -B scripts/check-agents-md.py --self-test`: 30/30 passed after the local-file fix in
  `cd8b59ff-a020-47c9-8cf8-54c05ada891a`; 33/33 passed after the pointer fix in `286bac63-21cf-4657-86e1-d40b772f73be`.
  These totals overlap; they are two observations, not 63 distinct cases.
- The real repository instruction check and changed-Markdown dprint checks passed. Runtime manifests have complete
  source and cleanup evidence, zero exits, and complete untruncated output. No failed runtime attempt occurred.

The final release-page run covered head `faae8269d6c68f990c8f7a12b70134312af2b060` plus its recorded working diff,
fingerprint `58a238b586a03edb9e2a4a757542cf6146bc654b960cbfdd3338577d8c2fe57a`. The 30-case run covered
`23553aaddee5a2e7a93db299df1bdf8171ecb0b1` plus its working diff, fingerprint
`9a3127db19dacf7f8eb805347c613b13377ee376a2496a1d99591c1d1f6bff13`; the 33-case run covered
`d205dd727128636383df5e2d92184e4fee415838` plus its working diff, fingerprint
`54e8c1f58abfb7da398ad888ee8ef95010e69e3f6e99fc3e0eb688b5065fec73`.

Reused: those successful results cover the final stack. Folding the normalization into #2417 preserved the tested
release script, and rebasing its descendants changed no instruction-checker behavior. Other post-test edits were
explanatory comments, inventory wording and outcome/PR bookkeeping. The final checker run covers the combined
instruction tree. Main changes examined during finishing were queue claims and delivery reports, without a tooling
interaction requiring a main rebase or more runtime tests.

Skipped: Rust, browser, desktop, installer, provisioning and release artifact builds; these scripts have no product
runtime or artifact-generation change beyond the review page itself. The requested named self-tests and source checks
cover the agreed scope. The Rust/browser delay checker was inapplicable because neither test family changed. No CI,
release, publication or deployment was started.

## Review gate's outcome

Each PR passed its single fresh-context gpt-6.1-sol reviewer at high effort, including the full test-authoring checklist
and size gate. The first reviewer also checked the portability correction. No findings remain. All three commit messages
passed separate cold reads, and a separate documentation pass checked each changed function, fixture and inventory
description. The PRs remain open drafts for the landing monitor.

### Landing

Landed as #2417, #2421 and #2422, merged bottom-up, with the same content the report describes, rebased onto this
round's main (see the end of these notes). It was the thirteenth of sixteen plans in this landing round, after the
twelve listed in the earlier plans' notes, most recently spec-scope-clarifications.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the twelve plans earlier in this round, and documentation
and plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. The
interaction that matters is whether the stricter instruction checker accepts what the other plans changed. It does: the
checker and its self-test passed on all sixteen plans stacked together, including every SPEC.md and SPEC_impl.md edit in
the round, and no local Claude instruction file exists in the repository. No other plan touches the release-notes review
page or the checker. This plan also changes two agent instruction files: the validation inventory and the run-all-tests
procedure; the landing monitor re-reads them before its next round. The only conflicts were in the review feedback
queue's index, where each plan removes its own entries; all removals are kept.

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

The report was written on a main from before round 107's session-identity change, so its statement that no rebase was
needed no longer describes what landed; the checker and page self-tests above ran on the combined stack. Nothing else in
the executor's report became untrue at landing.
