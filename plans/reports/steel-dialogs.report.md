## Since the last review

The two earlier drafts, Settings and menus, keep the same reviewed changes after rebasing. Two further drafts cover
confirmations and the session launcher. The recorded decision settled the previous block: Tab may follow the new reading
order, and Trash retains its pinned header actions.

## What this was about

Settings, session setup, confirmations and menus had inconsistent layouts. This plan applies the shared Steel and Steel
Light design: section icons and readable headings in Settings, consistent menu descriptions and destructive grouping,
shared dialog headers and answers, and labeled launcher fields with actions pinned below them. Existing host dialogs
already using the shared structure were reused.

The launcher keeps its fields and operations. Its footer shows what will launch where, with a compact Launch, Replace or
Restart button. Long forms scroll above it; long warnings and the inline YOLO question scroll beside the actions.
Initial focus, Escape, focus return, native choice controls, Enter-to-launch and request guards retain their existing
owners.

## Things you should know

Tab now follows header Close, fields in their existing relative order, then Cancel-first/main-action-last footer
buttons, as approved. The inline YOLO question stays inline. Trash keeps its totals, progress, Delete all and Close in
the header, with no footer or loss of archive-list height.

Long launcher summaries can truncate visually; the complete summary and action target remain accessible. The
save-as-template editor remains inside the form. Review found and corrected focus visibility when returning from that
editor and when a long YOLO question first focuses Cancel.

Visual corrections also align held session facts, managed-checkout fields and folder errors with the labeled rows, quiet
oversized helper text, and separate the permission warning from the next field. Disabled main actions keep the specified
quiet raised style. The phone footer and the bordered template editor retain their stacked layouts. The permission
explanation keeps its existing reading position; moving it would require a broader shared-control change. Notice
scrollbars use native styling when content overflows, without reserving empty space; visibility still depends on the
browser and platform.

Appshots show the real staged web UI in both palettes, at wide, constrained and phone widths. Native tunnel consent was
assessed from matching markup/CSS preview and source, with separate native compilation/controller tests; the images do
not prove native callbacks or AppKit behavior. The launcher images show the actual managed-checkout editor, while
ordinary browser tests cover repository preview behavior.

## Open questions and possible follow-ups

No required decision remains. The existing TODO entries revisiting keyboard order and Trash remain available for the
maintainer; they do not block this plan. The broad sentence-case pass and removal of the umbrella Steel TODO entry
belong to the next plan.

Pasting a session name with a line break makes Rename reject the name and show an error. The field keeps the complete
entered text for correction, but its existing one-row display can look cramped; moving the caret through the text
reveals the parts outside that view. This plan preserves that field sizing. Existing per-host Trash spacing and
untouched label capitalization were left for their own work rather than widened into this plan.

## The PRs

- [Settings navigation and headings, #2756](https://github.com/scode/farhelm/pull/2756/changes).
- [Command-menu descriptions and destructive grouping, #2757](https://github.com/scode/farhelm/pull/2757/changes).
- [Dialog layouts and confirmation answers, #2761](https://github.com/scode/farhelm/pull/2761/changes).
- [Session launcher rows and pinned actions, #2762](https://github.com/scode/farhelm/pull/2762/changes).

All remain drafts; the landing monitor owns merging them.

## Checks run, reused and skipped

Runtime checks used the recorder, pinned tmux where needed, zero retries and bounded selections. Browser behavior
selections ran on Chromium and WebKit with three workers; appshots use their normal single-worker staged fleet.

Commands were selected `cargo nextest run -p farhelm-ui --lib`,
`cargo nextest run -p farhelm-ui --features desktop -E <tunnel tests>`,
`npx playwright test <affected specs> -g <affected cases>` and
`scripts/docs-screenshots.sh --only visual-review --no-build`. The run IDs below distinguish the actual selections and
results; counts from overlapping selections are not added together.

- Settings: UI library 805 passed (51db0d5e-14a7-425e-bc6f-b8f0d26958a7); Settings browser 68 passed
  (522bfb0e-201b-4e6c-abe8-ff56466c1a5d), affected callers 22 passed (4ca8e49a-a731-46b8-a792-4eb0d10de888), corrected
  Settings 16 passed (039f9469-4f3c-45a1-b9fc-3d6e50d70d0b), final headings four passed
  (cb1ac041-f8c6-4a89-b9f5-2cfe56953846).
- Menus: 22 menu units passed (4ab85949-f6e7-4c0f-a371-46f77a51874a), geometry four passed
  (6338a5bd-811a-4e1b-923f-54b1f2ff9696), behavior 30 passed (72b5da6e-1054-4e27-8728-58616d7e63fc), corrected template
  four passed (8363da14-5dbf-4b8b-aab9-b01867cf999d), final affected cases ten passed
  (be69d882-faa6-48be-8768-83d00719fe75).
- Confirmations: UI library 805 passed (8177fa16-a156-4da9-b519-d9fa67857d00); six desktop-feature tunnel tests passed
  (728ebe9f-0b39-42ea-a442-4f1df70b34fd). Selected browser checks passed 46 (a0bcc317-892d-4093-8e8b-460f67bd7f84), six
  (4f7ccc34-2ed2-463f-adf8-d20f1344fc57) and the two corrected phone cases (471a3fc4-3bfb-4504-bad2-44395147a755).
- Launcher: 52 form units passed (780e0381-c6f0-481b-95a7-99d9d4bcbc72). Initial browser selection had 59 passes and
  seven failures (694e5e7d-68ae-4f96-bcd8-98475a32ddb9); corrections addressed dropdown setup and template focus
  recovery. The narrowed correction run had 24 passes, one intentional WebKit screenshot-matrix skip and one
  clipped-Cancel failure (a3ddd9a1-86a4-4ec3-8372-9d266a0acae4). After the scoped scroll-margin correction, that exact
  YOLO case passed on both engines (0190390e-92ee-4ed6-8452-eb7a4ade9b20). Following the visual CSS corrections, all
  eight selected footer-geometry and YOLO focus-visibility cases passed on both engines
  (340c01ac-29ac-4b2e-8d98-472bc84f2a30). Unaffected passing cases are reused; no clean pass of the entire original
  selection is claimed.
- Final launcher notice correction: the two YOLO keyboard/focus-visibility cases passed on both engines
  (e8b84785-5f30-445d-9104-d8d77d24c6d0), checking the remaining width/wrapping risk after removing the reserved
  scrollbar gap.
- Normal appshots: Settings 60 PNGs from five passing staging/capture cases (a2678b3e-3d9b-4583-86f2-f0795c0c75a8);
  menus 56 PNGs/five cases (ac476805-6377-412d-af9e-de04dcd7a61a); confirmations 52 PNGs/five cases
  (92bb7812-0cc3-4a8e-9e46-d6175f9fd6f4); final launcher 60 PNGs/five cases (b937b6ce-3789-44d4-a241-40bc85b6e16b).
- Applicable UI Clippy, web builds, desktop-feature compilation, Rust formatting, changed Markdown formatting, changelog
  format, TypeScript parsing and the test-delay checker passed. The delay checker found 322 annotated delays and none
  missing a reason.

Earlier Settings/menu evidence covers heads 36238b70c1e58a1ae0f2ebe980c6e4c39bcbba71 and
6d3785de047a3c5229a94597f8300ea56f2bf0dd. Their rebases have empty interdiffs. The intervening main changes contain
instructions and queue/review records, without a product, dependency or goal interaction. Later launcher changes do not
alter those reviewed surfaces. The launcher adds no CLI source changes, and no fresh CLI build is claimed for that unit.
Launcher focus/CSS corrections do not change the 52 pure form tests' business logic.

The final stack heads are b03bc5c47a6cde1e65867447e0c2e15115cb23b5 (Settings), 61b55141879f1c84497da998265bd0e41fb10df3
(menus), a4eeb7f78662cf8004f229c144827c5b624c785a (confirmations), and 618e6d0013b90b1c3909c7b61d1cdd5918f26ea9
(launcher). Launcher checks used the working tree now committed at that last head; earlier runs are reused only for
unchanged behavior as described above. The final CSS build passed, and the final two-case browser run and appshots cover
its changed notices.

The two failing launcher browser selections identified above remain retained; later passing selections are separate
observations. These authoring failures were fixed during this execution and were not logged as latent flakes. The WebKit
screenshot-matrix skip supplies no coverage for that matrix, while the ordinary keyboard and interaction checks run on
both engines.

The full runtime battery, CI, installer, release and unrelated backend checks were skipped because the bounded
presentation changes had targeted coverage. No macOS/AppKit runtime capture was performed. No live installation was
modified.

## Review gate outcome

Settings, menus and confirmations passed their separate fresh gpt-6.1-sol/high code gates and claude-opus-5-5/high
multimodal gates. All images were delivered as actual successful image reads; findings were corrected or explicitly
declined with reasons in the working log.

Launcher code and visual reviews passed. The final visual follow-up confirmed all three remaining alignment corrections
across 28 changed images; the other 32 views were unchanged and reused from the preceding complete visual review. Fresh
own-model scope reassessment found no unnecessary mechanism: the focus fixes reuse the shared facilities. No required
finding remains outstanding.
