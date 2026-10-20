# Launch-sequence hints and visibility

## What this was about

Incomplete quick launches could tell you to use the sequence prefix you had just pressed. Templates could offer the
quick-launch mark for a command template with no command, and a sequence whose template disappeared could open Templates
without explaining why. The queued-template pill was also easy to miss at the terminal's bottom edge.

The requested two-PR split is preserved. A number key keeps its existing partial-template explanation. An incomplete
ordinary quick launch now says the template needs more choices; an incomplete sequence says the combined setup needs
completing. Templates, the launch check and agent template writes share the active-kind rule: an agent launch needs an
agent; a command launch needs a nonblank command. An invalid marked template can still be unmarked. Missing named
templates reveal New with the existing explanation, and sequence bindings refresh when the prefix starts.

The pill is larger and amber, centered over the terminal or empty main area, with a soft halo and one 200ms entrance.
Reduced motion disables the entrance. It remains non-modal and ignores pointer input. While the session launcher is
open, it retains its old bottom position. The specs and the Launch templates docs page match, both changelog fragments
are present, and both TODO entries are removed.

## Things you should know

Review found that refreshed bindings could reach the launch worker before the pill displayed them. After the required
scope reassessment, physical Enter was made to capture the bindings published alongside the displayed labels. A hidden
copy remains available before the first pill render for rapid input. This uses the existing key snapshot, with no new
preferences subscription; Enter does not refresh bindings, and unbound keys stay unbound.

The Mac placement uses the agreed fixed 9px correction. The browser check exercises the Mac CSS class, not an actual
AppKit window. Conditional bands can still shift the exact terminal center slightly, as accepted in the plan.

Three failed test attempts remain retained separately. Run `b9555cb2-28fe-41b4-b7db-6f4886da856e` failed both engines
because the new focus assertion targeted a wrapper rather than its input. Run `19086655-6bf5-4265-a0ba-90f209ad675b`
passed Chromium and failed WebKit because the new assertion assumed the details bar was exactly 30px high. Narrow WebKit
run `f853c296-bcdd-41b8-9063-653645e4f738` established a 31px bar with the correct existing 44px bottom reservation. The
test now checks that reservation and actual clearance; production placement needed no correction. These were
same-session test mistakes, not latent-flake claims. The final two-engine pass is a separate observation.

## Open questions and possible follow-ups

No unresolved decision blocks delivery. The existing docs screenshot showing Templates help is now dated and can be
refreshed during ordinary docs screenshot work. The published v0.30.0 sequence screenshot and its test were left frozen.

## PRs

- [#2425: explain incomplete quick launches](https://github.com/scode/farhelm/pull/2425/changes) — hints, shared
  eligibility, missing-template explanation and binding capture; draft head `0b5caa633fd51b349b7e55c18b32e56ddf54f5ed`,
  based on main.
- [#2426: highlight launch sequences](https://github.com/scode/farhelm/pull/2426/changes) — pill placement and
  appearance; draft head `f013f1d9d47f412b89b77394e7d1107454454d3d`, based on #2425.

## Checks run, reused and skipped

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, Markdown formatting, changelog-format validation and the test-sleep
  checker passed. The sleep checker inspected 304 delays with none missing a rationale. The instruction-reference check
  also passed. Native CLI and release web builds passed; the web build was repeated for the pill.
- Recorded UI/proto library and focused helm shortcut-policy tests passed 732 cases in
  `748b08c7-6520-4aec-a0aa-0f3d831eecd7`, with four nextest slots and zero retries. The other 1,208 helm cases were
  selected out, not executed. This covers banner choice, active-kind eligibility, invalid-template unmarking and
  captured binding decoding.
- The JS harness passed 311 cases in `120feb0c-3193-4f77-9be4-ee31c84c92c6`; the changed shortcut asset then passed 33
  in `8a2bc491-9691-4a96-8478-05eb684be39f`, including one added case. These cover 312 distinct cases, not 344. The
  pill's CSS token, contrast and animation selection passed 37 in `6ef251e6-ce9b-4c02-814b-7cb4bd08c5a3`.
- Recorded browser selection `quick-launch`, `launcher-template-shortcuts` and `template-list-shortcut` passed 94 cases,
  47 per engine, in `da6da4f9-2160-4bcc-bde5-b9cafb8e2472`. The targeted Templates composition-banner/tooltip case
  passed once per engine in `ae912a38-fba1-4214-ae53-835bf9d85763`. Chromium and WebKit ran with one worker and zero
  retries. These selections cover the changed launch entry paths and required hint/binding regressions without unrelated
  Templates CRUD.
- The final pill geometry/cancellation case passed once per engine in `c0819ef4-3650-42c2-8120-2cb7bb8b6aaa`, covering
  normal centering, Mac CSS offset, reduced motion, pointer behavior and launcher placement. The retained centered
  screenshot was also inspected visually.
- Successful runtime records have complete source identity, retained output with EOF and no truncation, complete cleanup
  and no runtime `SKIPPED` marker. Runner-specific Rust/browser reports were complete. Controlled tmux runs used the
  checked pinned 3.7c executable.
- Tested-revision mapping: the original 311-case JS run used base `6a2c292d6679faf4ed24f56e035696e437d9aad2` plus
  fingerprint `b375811f3989b1f06eb042ede4d734496354f30c490c787dae456732ee693e88`. Corrected first-PR Rust, shortcut JS
  and browser runs used that base plus fingerprint `f4ebd196a25209f6e6891f773bf4ae73c2304cff2177a10346fcfd0e93188ee3`;
  publication changed history metadata without changing that source. Final pill geometry used #2425's head plus
  fingerprint `1f4f68eb8d69bb7a8449a51ad91cf79d40cbfbef3e5303268224bc240ac7bdc0`, the source published as #2426. Earlier
  JS coverage is reused only for unchanged assets; the pill CSS selection covers its new tokens and motion. First-PR
  Rust/lint/desktop checks remain applicable to the unchanged launch logic; the later web build covers the added
  launcher-context attribute.
- `bun install --frozen-lockfile` and the website build passed, including internal links. It ran on base `6a2c292d` with
  the first slice's working docs edit, before the binding-capture correction; the Launch templates page was the content
  published in #2425. No separate full-tree fingerprint was recorded for this build. It is reused for the second PR's
  prose-only paragraph, which adds no links or components. No preview was started for this unattended incidental docs
  edit. Main's intervening changes through `87a7a913` were queue/report bookkeeping, read in full, with no
  implementation interaction requiring a rebase or rerun.
- The full browser battery, unrelated Rust targets, installer/provisioning tests and actual desktop runtime were skipped
  because these changes do not affect their behavior. Browser WebKit coverage does not claim native desktop timing
  coverage. No live installation was changed.

## Review gate outcome

Each PR received its own fresh-context gpt-6.1-sol/high review. The first review's binding-publication finding was fixed
after scope reassessment and accepted on follow-up. The second review accepted the pill and the corrected launcher
geometry assertion. There are no unresolved findings. Commit/PR wording cold reads passed; the delivery report receives
the planning flow's separate cold read before delivery. Both PRs remain open drafts for the landing monitor.

### Landing

Landed as #2425 and #2426, merged bottom-up. It was the last of sixteen plans in this landing round, after
host-settings-focus. Its content is what the report describes, rebased onto this round's main, with one TODO.md conflict
resolution described below.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the fifteen plans earlier in this round, and documentation
and plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan.
Three touch the same ground:

- template-list-current-labels, from round 107, refreshes the template list's shortcut labels and states that picking a
  template uses whatever template currently holds that name. This plan uses the same refresh for sequence bindings, and
  when Enter launches the sequence, each bound template is looked up again by its current name, which follows that rule.
  A template name that no longer exists still produces the existing "no longer available" explanation in New, now for
  sequences too.
- launcher-retry-identity, earlier in this round, keeps a retry on the same attempt when an unchanged template is
  applied again. Each template in a sequence is applied the same way a single template is, so a retry keeps its attempt
  and a sequence cannot lose it.
- terminal-link-and-copy-fixes, earlier in this round, refuses link clicks while another Farhelm surface covers the
  link's destination details. This plan moves the sequence pill to the middle of the terminal; see the follow-up below.

#### Fixes made during the landing

- TODO.md: host-settings-focus, landed just before, removed a neighboring entry, which conflicted with this plan's
  removal of its two entries. The resolution removes all three (#2426).

#### Possible follow-ups

- While a quick-launch sequence is waiting, its status pill sits above the terminal, and now in its middle rather than
  at the bottom edge. The pill is not on the list of surfaces that count as covering a terminal link, so a link whose
  destination details are partly hidden under the pill still opens when clicked. The gap predates this plan; centering
  the pill makes the overlap more likely. It only matters if the user clicks a link while a sequence is pending, and the
  pill does not take pointer input. Possible fix: add the pill to that list. This was found by reading the code and was
  not checked in a browser.

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
needed and its focused checks no longer describe what landed; the combined runs above supersede them. Nothing else in
the executor's report became untrue at landing.
