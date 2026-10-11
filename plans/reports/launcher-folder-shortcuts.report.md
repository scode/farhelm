# Launcher folder shortcuts

## What this was about

The session launcher showed recently used folders as unlabeled dotted links below its folder field. Long paths lost
their endings, making sibling folders look alike. The maintainer chose a dropdown on the folder field, with eight recent
folders and separate home resets, rather than a labeled list or chips.

The editable folder field now opens that dropdown on focus or an arrow key. It shows the host's eight newest remembered
folders until the user types; typing filters them by case-insensitive substring. Long labels keep the distinguishing
end, and hover and the accessible name carry the whole path. The reset group explains home as `~ on this host` and local
home as `~ on this machine`.

## Things you should know

Opening history highlights nothing. Enter still launches the typed path until an arrow key highlights a suggestion;
Enter then chooses that suggestion without launching. Escape closes the list first, and a second Escape can close the
launcher. Tab, blur and choosing close the list; typing reopens it. Managed checkouts have no folder dropdown.

Mouse and keyboard choices share the existing destination guards. A remembered folder remains tied to the host
installation that offered it, stale folder and browse callbacks still refuse, and local home deliberately pins the local
host even when it was already selected. Choosing restores the unfiltered recent list for its next opening. The former
current-folder pressed marker is gone; highlight now means keyboard navigation.

Execution found that putting the list in normal layout could move the centered launcher between mouse-down and mouse-up
when blur closed it. A pointer press on Launch then missed its submission. The list now floats below the input using the
launcher's existing model/search pattern, with bounded scrolling. A regression test measures Launch before and after
that blur and checks the actual create request. Both browsers also prove a real phone click chooses the full path and
arrow navigation reveals the last reset.

The destination screenshot capture was updated to open and target the list, and its documentation alt text was updated.
The documentation also states the directory dropdown's Enter exception beside the launcher's other keyboard choices.
Published screenshots still depict the earlier links; this plan did not regenerate or publish images or deploy the
website. The screenshots used for validation are private test artifacts.

## Open questions and possible follow-ups

No maintainer decision remains. A future documentation screenshot refresh can replace the published destination image
with the dropdown; that publication was outside this plan.

## The PRs

- [#1874](https://github.com/scode/farhelm/pull/1874/changes) — grouped folder dropdown, its guard and interaction
  tests, specification, changelog fragment, and removal of the covered TODO entry. This executor leaves it draft for the
  landing monitor.

## Checks run, reused and skipped

Final browser checks used `scripts/record-test-run.py`, pinned tmux, one worker and zero retries, on Chromium and
WebKit.

- The new dropdown test and the two clone cases that exposed the pointer regression: six passed, run
  `8e98ae7b-4891-4a96-887f-fcfcd23ee188`. This covers filtering, the eight-row cap, keyboard choice versus typed-path
  Enter, Escape, tooltip, managed-mode absence, pointer Launch geometry, real phone choice and reset reachability. All
  six desktop/filtered/phone screenshots were inspected; sibling endings remain distinct.
- 139 passed and one intentional skip in run `c5fe7167-e7dc-4604-86a2-8d9263c8477c`: selected launcher Enter/tabs,
  managed checkout, composer search, remaining clone cases, visual captures, template application/save, changed
  history/reset and destination-authority cases. These check the focus/click interactions of opening history throughout
  the launcher. The two clone cases above were excluded; the dropdown test also matches the history selection and runs
  again. Chromium passed 70 cases; WebKit passed 69, with the existing Chromium-only visual matrix skipped.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo check -p farhelm-ui --features desktop` passed on the final floating implementation. The desktop check is
  compilation evidence, not native app interaction evidence.
- `python -B scripts/check-test-sleeps.py` with its isolated interpreter passed: 285 deliberate delays, zero missing
  rationales. Changed Markdown passed `dprint check`. `python3 releasing/check-changelog.py format` passed with nine
  valid fragments.
- `cd website && bun install --frozen-lockfile && bun run build` passed on the rebased combination: 32 pages and all
  internal links valid. The subsequent plain-word directory terminology correction leaves the MDX structure and
  references unchanged, so that successful build remains applicable. Root dprint excludes MDX; no MDX formatter pass is
  claimed.

Reused: `cargo nextest run -p farhelm-ui --lib -E 'test(folder_dropdown)'` through the recorder passed both helper
tests, run `90e2af42-51ae-4962-90e7-32935d48cddb`. It covered main `baa90a5d` plus the implementation before the layout
correction; the filtering and escaped-tail helpers and their tests have not changed since. The matching CLI and final
web builds were reused for browser execution. Final browser evidence covered main `baa90a5d` plus the corrected dirty
implementation; comparison against that tested revision confirmed all affected runtime and browser-test files are
unchanged after rebasing. The pushed revision is `e560ee4016fc`, based on main `0c875c08`.

Careful rebase inspected the intervening documentation callouts, screenshot tooling and fixtures, published metadata and
page refresh, instructions and unrelated TODO entries. The destination capture and its page had textual conflicts; both
were resolved by preserving upstream's destination-kind callout and placements while adding the new list selector,
description and keyboard exception. A later TODO checklist planning tool and its tests landed independently of the
launcher. None changed product runtime or specifications. The combined site build covers the shared documentation
integration; no runtime rerun was warranted for these upstream changes.

Earlier attempts remain failures, rather than being replaced by later passes. The initial integration run
`3dc47c01-0971-4ad0-b690-cc193dab7bd5` was stopped after two clone failures: seven passed, two failed, one interrupted
and 112 did not run. Exact clone reproduction `27600244-b212-40b8-8a69-aceca3aa4bd9` failed. The focused pointer
geometry run `e134c24b-eba2-496f-9ce3-f45d74000ef8` failed on both engines before the floating correction. Their
evidence is retained privately and the corrected cases pass in the six-case run above. This was a regression introduced
and fixed during execution, so it is not recorded as a latent flake. Earlier normal-layout browser passes are not
claimed as final-layout evidence.

Skipped: the full Rust and browser batteries, native desktop runtime, installer/provisioning/release checks and
unrelated JS harnesses. The changed code is the launcher UI, and the selected interaction tests plus compilation and
pure-helper tests cover its concrete risks. Documentation capture regeneration/publication and website deployment were
expressly outside the plan. No hosted CI was requested; this repository does not start CI on a PR or merge.

## Review gate's outcome

The prescribed fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, language idioms, the
acceptance criteria and the full test-authoring contract. Its two initial findings were fixed: the docs capture opens
the new list, and the already-local reset test proves explicit host pinning through template inclusion. Follow-up
reviews of the floating correction, phone pointer addition and rebased documentation returned no findings. An
independent scope review accepted the existing floating-list design; its required phone scrolling and hit-testing
evidence passed in both engines. Source review supplied no runtime evidence; the executing session ran and inspected the
checks above. Commit and PR wording passed its fresh cold reader.

Implementation and investigation stayed local under the no-workhorse requirement. Only the required source/scope and
wording reviews were routed through galaxy-brain; the planning system's resume and report readers run separately.
Private session evidence is retained under UUID `a63f6b2c-45f7-4830-bf84-ea76de63472d`; exact native model attribution
and usage counters were unavailable, and delayed recording gaps are recorded there.
