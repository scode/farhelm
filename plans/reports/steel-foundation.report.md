## What this was about

This is the shared foundation for the Steel redesign. Farhelm gains blue-gray dark colors, a light counterpart, solid
periwinkle selections, and Inter for interface text. Paths, commands, versions, ages and terminal text keep JetBrains
Mono. Settings gains Appearance: System follows the device, while Dark or Light overrides it on that device. The choice
stays on the device; the helm does not store it. The terminal keeps its existing dark background and colors.

Existing buttons, switches, short exclusive choices, menus and framed dialogs now share the Steel styling. The stack
also adds a reusable, unpublished appshot capture on the ordinary docs fleet. The reference mockups are unchanged. The
separate Steel plans still own the host rail, session toolbar and tunnels, later dialog layout work, and sentence case.
This plan leaves their TODO entry in place.

## Things you should know

Keyboard and operation contracts stay with their existing owners. The launcher keeps its native permissions radios and
Resume checkbox, including Enter-to-launch behavior. Disabled controls retain their explanations and operation admission
checks. Delete and Replace confirmations put Cancel first while retaining their existing inline ownership. Appearance
changes preserve the mounted terminal and apply without reloading the page.

The visual review found a real light-mode focus problem and washed-out disabled labels. Those are corrected: focus on a
filled control draws inside its fill, and unavailable filled actions use readable neutral styling. Settings labels and
descriptions are grouped more clearly; notification Save and Send test share a footer; technical values use mono
consistently; host-menu versions stay on one line. A checked switch's focus ring in Light is visible but subtle; the
visual reviewer considers that nonblocking.

Several suggestions were deliberately declined. An empty Feedback form keeps the common quiet disabled-action style
instead of a faded primary. Template editing selection covers its own control, not the independent quick-launch and
shortcut controls beside it. Menus retain their existing first-item keyboard focus policy rather than changing what
opening a menu means. Existing inline confirmations were not replaced with new modals. Broader wording and layout
changes remain with the later Steel plans.

During investigation, an artificial unpaced native script sometimes showed stale Settings colors around immediate
palette changes and closure; leaving the dialog open allowed it to settle. There is no ordinary physical-input
reproduction or demonstrated product fix for that observation. Its private evidence is retained. Following the
maintainer's direction, the expanded video, frame extraction and timing investigation were removed. Validation uses
normal appshots and the ordinary bounded desktop integration test.

## Open questions and possible follow-ups

The visual review uses ordinary representative dark/light appshots at wide and constrained widths. Active host
operations, pending Trash, loaded SSH help, the installed native updater and successful uninstall planning do not have
pixel captures here. Their shared styling is reviewed from code and photographed counterparts; those unobserved states
are not claimed as visually tested. No live installation was changed. Native macOS drawing and live system-appearance
notifications also remain outside the Linux/browser evidence.

The subtle checked-switch focus ring in Light and the artificial rapid-native-change observation can be revisited if
ordinary usage supplies a concrete reproduction. They are recorded limitations, not claims of a fixed compositor issue.

## The PRs

- [#2728: unpublished visual-review appshots](https://github.com/scode/farhelm/pull/2728/changes).
- [#2729: Inter interface typography](https://github.com/scode/farhelm/pull/2729/changes).
- [#2730: Steel dark palette and solid selection](https://github.com/scode/farhelm/pull/2730/changes).
- [#2731: device appearance choice and Steel Light](https://github.com/scode/farhelm/pull/2731/changes).
- [#2732: shared controls, menus and dialogs](https://github.com/scode/farhelm/pull/2732/changes).

All five are a linear stack of drafts. No executor merge or ready transition was performed.

## Checks run, reused and skipped

Current final checks: desktop-feature Clippy for the UI library and binaries passed; workspace Rust formatting, shell
syntax and warning-level ShellCheck passed; the changed validation inventories pass dprint; the instruction checker
reports no errors and two existing citation warnings; all 322 inspected test delays have reasons; changelog format
validation passed. These cover the simplified native fixture, the final capture handoffs, and their documentation.

The final static commands were `cargo clippy -p farhelm-ui --features desktop --lib --bins -- -D warnings`,
`cargo fmt --all -- --check`, `bash -n scripts/desktop-smoke.sh`, `shellcheck -S warning scripts/desktop-smoke.sh`,
`dprint check` on the three changed validation inventories, `python3 scripts/check-agents-md.py`,
`python -B scripts/check-test-sleeps.py`, and `python3 releasing/check-changelog.py format`. Runtime commands below used
the recorder. Rust selections used four nextest slots and zero retries with pinned tmux 3.7c. Browser behavior
selections used Chromium and WebKit, three workers with one owned stack each, and zero retries. The visual appshot
selection used one Chromium docs-fleet worker; the native smoke used one owned Linux Xvfb/openbox desktop at a time.

The normal single-layout appshot sequence passed, including staging and all 46 surfaces, in run
`fb29b3b6-3770-4435-be34-6f61c1734922`. It verifies the corrected closed-panel focus handoffs. The earlier four-layout
attempts `9bf1de9c-8ff6-40f2-afe6-e5fd99b25e09` and `8ee1d351-2c44-4d91-b5df-998179dfd0cc` remain failed attempts:
keyboard-owned help survived pointer movement and stopped a later capture. The latter still saved 168 valid images of 42
surfaces across both palettes and widths. Those saved images supply the representative visual review; the failed run is
not described as a passing capture. One overly anchored grep selected no tests in a separate retained attempt.

Ordinary native appearance smoke passed in run `c772ca7a-6aec-4dff-89ac-9bbeb2e024fe`, on the final stack tip
`19e9eadd86ae4ff826544469997991b7cd017481`. All four fresh/stored dark/light cases passed, covering startup palette,
Settings choices and persistence, fonts, and the same live terminal's ordered output and input. The run took about eight
minutes including builds. It retained one final client image per case; it makes no transient-paint claim. Its child
command was `DESKTOP_SMOKE_APPEARANCE=1 scripts/desktop-smoke.sh`. The appshot child command was
`npx playwright test -c docs-shots.config.ts docs-shots/visual-review.spec.ts -g 'dark wide' --reporter=line`.

Reused checks retain their original scope rather than claiming a new full-suite pass. The existing UI library run
`85b323ce-5cd5-4442-a7c5-af3377b0dded` passed 783 tests at the Inter step, with 29 focused typography tests checked
after the relevant corrections in `572f1230-80d1-4622-90bf-3164de9a84d1`. This is baseline and focused evidence, not a
claim that all 783 were rerun at the tip. The pure asset JavaScript run `3570be6e-81dd-4a26-8da5-1fdbf4f18eaf` passed
358 tests. Final CSS checks reuse 52 unchanged passing cases from retained failed run
`f82d1b69-c1f2-4744-8f06-2b09795331f7`; its outdated source-selector binding was corrected and the exact case passed in
`c82af871-59f3-4828-a1d9-fe4d0dda9dd7`.

The touched Settings, buttons, Feedback and notification browser cases produced 106 unchanged passes across Chromium and
WebKit in retained failed run `27888264-2b2c-4b62-ac03-8a5ac5bfb1bb`. Its two failures were a newly added test's focus
handoff after Cancel; that exact case passed on both engines in `46ff0859-dca3-4b45-a5b1-34f5c971c07b` after the fixture
correction. The 30 appearance/launcher/OMP cases in `2bc5b36d-66c8-463f-83cb-1dab3ee9e5ad` and six geometry cases in
`c5adb237-4dcb-4c3c-abb6-fde26aa42403` remain applicable. Later changes preserve the product behavior those cases
exercise; final edits only simplify the native smoke and correct capture inputs. Failed runs stay retained and are not
converted into passes.

These are the recorded source identities and exact selections for the reused commands. Hash prefixes identify the
recorded HEAD and complete source fingerprint; a fingerprint also covers uncommitted edits, so HEAD alone is not
presented as the tested tree.

| Run prefix | Recorded HEAD / source fingerprint | Child command or exact selection                                                                                                                                         |
| ---------- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `85b323ce` | `6d83cb62cc2e` / `0d9960f1c5e3`    | `cargo nextest run --locked -p farhelm-ui --lib`                                                                                                                         |
| `572f1230` | `6d83cb62cc2e` / `2dde68f8fe78`    | Same command, `-E 'test(/list::templates::tests::\|hosts::tests::destination_details/)'`                                                                                 |
| `3570be6e` | `4a9f5fb5ee2f` / `13258aefa2da`    | UI asset harness, `node --test`; default discovery, no external fixture                                                                                                  |
| `f82d1b69` | `a657ac14bb3e` / `5dba6cf50d05`    | `node --test app-css-contrast.test.js app-css-tokens.test.js app-css-animations.test.js`; one process                                                                    |
| `c82af871` | `a657ac14bb3e` / `aa097e589d39`    | `node --test --test-name-pattern 'solid selection uses inverse roles' app-css-contrast.test.js`; one process                                                             |
| `27888264` | `a96f1b590601` / `bbf08887e872`    | Playwright Settings, buttons, Feedback and ntfy-settings specs; both engines                                                                                             |
| `46ff0859` | `a96f1b590601` / `7290487c86f4`    | Playwright buttons spec, `-g 'unavailable filled actions stay quiet and compact questions keep answer order'`; both engines                                              |
| `2bc5b36d` | `89b927aa0453` / `ca572f5d267f`    | Selected native short-choice keys, refused Settings drafts, template save/replacement focus, live appearance, launcher Enter and eight-item OMP menu cases; both engines |
| `c5adb237` | `89b927aa0453` / `a75a645c56ea`    | Sidebar spec: session flyout beside its row, last-visible-row menu, retained refusal inside bottom-row menu; both engines                                                |
| `60581d40` | `155ae6e02c6b` / `c71fc768a7a3`    | `DESKTOP_SMOKE_APPEARANCE=no-initialize scripts/desktop-smoke.sh`; one owned Linux desktop                                                                               |

The Inter runs remain evidence for the font-role and formatting behavior they exercised. Subsequent shared-control work
is covered separately by the CSS, browser and native selections. The earlier full JavaScript run supplies unchanged
asset behavior; the final CSS correction has its own later selection. The browser runs retain their keyboard,
persistence and geometry coverage because later styling and test handoffs do not change those contracts.

No new full Rust or browser battery, release test, installer run, deployment or live-service restart was needed. The
affected behavior already has focused evidence. The prior missing-initialize fault run
`60581d40-a974-402f-9c2b-8fd57df90a33` remains separate Linux lifecycle evidence; it is not a visual pass or macOS
proof. Its creation deadline and owned-supervisor assertions remain in the simplified fixture, with only the abandoned
video argument removed.

## Review gate's outcome

Each PR received the required gpt-6.1-sol review at high effort. Findings were fixed or explicitly decided, including
the capture helper's unsafe proposed outside click, which was removed. The final native simplification and capture
handoff reviews have no findings. The required Opus 5.5/high reviews cleared the preceding typography, palette and
ordinary appearance work. The final shared-controls review uses ordinary representative appshots and code-only judgment
for the coverage limits above. Its final verdict is a representative visual pass, with the subtle Light focus-ring note
accepted as nonblocking. It does not claim inspection of every captured image or the uncaptured states.
