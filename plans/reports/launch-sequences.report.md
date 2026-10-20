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
