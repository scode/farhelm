## What this was about

In the installed desktop app, checking for updates required finding the help menu or the local host's Update action,
although progress and results appeared beside the version. Automatic updates could also stop working with no visible
failure. The maintainer chose a dim trailing refresh glyph for the version's check action, and the existing inline
failure notice for automatic trouble: installs fail visibly at once; latest-release probes must keep failing for three
days before warning.

The version now checks for updates when no update waits. Hover and keyboard focus brighten it and turn the refresh glyph
a quarter; visible checking/installing spins that glyph with the existing progress words. A waiting update keeps its red
arrow and Restart to update / What's new menu. The web UI and builds without an updater keep plain text.

## Things you should know

Automatic work shows no progress unless the user joins it. Failed downloads after a successful latest-release probe
count as install failures and show immediately. Probe failures measure the current failure streak, not time since the
last successful check: sleep, quit, a waiting update and automatic updates being off do not start a clock by themselves.
A successful probe clears that streak, including before a later install failure. A fully successful check clears a shown
failure. A user's check takes precedence with its own progress and outcome.

The streak start is saved with exact seconds/nanoseconds in the existing desktop state file and survives restarting.
Missing history means no streak. A future timestamp or backwards clock resets it to now; re-enabling automatic updates
clears it. Unreadable or malformed state supplies no known history. Failed saves are logged; checks and in-memory
history still work, but an unsaved change cannot be promised across a restart. Older releases ignore the added field.

An automatically shown failure remains through unsuccessful retries until success or dismissal. Clicking it hides its
inline words; the reason remains in the hover. A later qualifying failure shows again, including the same reason.
Selecting the version retries. Verification refusals retain the existing must-be-reinstalled warning, and the daily
schedule and retry interval are unchanged.

## Open questions and possible follow-ups

No unresolved product decision blocks delivery. Once a release carries this feature, inspect the readout in the
installed Mac app. The event tests prove the production check control's callback, and Chromium/WebKit checks prove the
shipped CSS and bundled glyph; neither runs the installed native Mac shell. No live installation was touched.

## The PRs

- [#1988 — check from the desktop version](https://github.com/scode/farhelm/pull/1988/changes), draft, click action and
  refresh affordance, with its specification and changelog fragment; head `a61b3bfc7ef759a4bf2aa9ef206633935b3cbeda`.
- [#1989 — visible automatic failures](https://github.com/scode/farhelm/pull/1989/changes), draft on #1988, failure
  visibility and persistence, its specification and changelog fragment, and removal of both covered TODO entries; head
  `0b282c91bfcbb1aebb6fdae47efc99c441b40f5a`. No PR was marked ready or merged by this executor.

## Checks run, reused and skipped

PR 2's focused desktop updater, state and readout selection passed 69 tests (`00774b3d`), then the full desktop package
passed 587 with zero skipped (`d4a7af4f`). These cover the exact three-day boundary, long gaps, manual/automatic
success, sticky notices, dismissal and equal later failures, joining, backwards/future clocks, real-file restart,
missing/unreadable/malformed state, failed writes, re-enable during a probe, and verification refusals. Shared readout
and app-bar tests after the notice-identity rename passed 13 (`82db41d9`). The review strengthened the manual
replacement's retained activity and the first joined automatic failure's history assertions; both exact tests passed
(`5d5d7173`). Production behavior was unchanged, so the earlier package pass still covers the other cases.

PR 1's native render/click proof passed (`005308de`), and its shipped-CSS affordance/motion proof passed all four cases
on Chromium/WebKit (`3fb5d39d`). Its asset JavaScript harness passed 241 (`e7e1caf8`), and bundled-glyph screenshots
from both engines were inspected (`aeb52e9e`). These are reused because PR 2 changes no markup, CSS or asset JavaScript.
The earlier desktop selection's stale tooltip assertion failed (`b00fbc6d`), was corrected, and passed on exact retry
(`9dbd36cb`); the later full desktop run also covers it. Early render and browser fixtures had setup errors; those were
corrected narrowly, and their failed runs remain recorded privately. Later passes do not relabel them.

`cargo fmt --all -- --check`, `cargo check -p farhelm-ui --features desktop`, workspace all-target Clippy and desktop
all-target Clippy passed. The isolated source-delay checker inspected 298 delays with zero missing reasons.
Changed-Markdown formatting, changelog layout and diff whitespace passed. Careful upstream inspection found changes to
planning, TODOs and Mac release-test tooling, with no updater runtime or dependency interaction; the matching runtime
evidence remains applicable across that rebase. The later ntfy feature uses separate helm configuration and
settings-scoped CSS, and leaves the updater's setting handler, state file and work schedule untouched. Its spec
additions affect other sections. The final rebase preserved both features; a fresh desktop compile passed with their
additive modules together. Earlier updater runtime and glyph/CSS evidence remains applicable.

No installer, provisioning, release, full-workspace or whole-browser battery was needed. Web readout markup stayed
unchanged, and the focused browser proof addresses the new CSS contract. The package run and transition/persistence
tests cover the changed desktop worker without using a real installation.

## Review gate outcome

PR 1's required fresh gpt-6.1-sol/high reviewer found three gaps: a stale tooltip assertion, missing actual event/action
wording proof, and missing computed CSS affordance proof. All were corrected; its recheck found no unresolved finding.
The reviewer inspected source and supplied evidence, without rerunning tests. PR 2's fresh sol-high reviewer found two
test gaps: the manual replacement asserted only identity and could miss disappearing text; the join fixture already had
a streak and could miss history from a first joined automatic failure. Both were corrected, and its recheck closed both
with no remaining finding. It inspected the two-test output but ran no tests itself. Commit and PR wording passed
separate fresh cold reads. The independent report cold read passed.

### Landing

Landed alone in its round as #1988 then #1989. Since the stack was based, main had gained only queue bookkeeping. The
executor had already rebased over the ntfy alerts feature (#1986), which shares the settings dialog and the app's
stylesheet; the landing re-checked the combined tree (below).

#### Release-relevant

No helm or supervisor schema change, and no protocol change. The desktop app's own state file gains one optional field.
Older releases ignore it, and drop it the next time they write the file.

#### Fixes made while landing

An independent fresh-context review found nothing blocking. The landing fixed four points in #1989 before merging:

- A stale failure notice could stick beside the red update-ready readout. Trigger: an automatic install fails and its
  notice is kept, then the user installs the newer version some other way, such as running the installer from a
  terminal. Automatic checks hold back while an update waits, so nothing cleared the notice: the user saw **check
  failed** with a hover saying the install did not finish, next to a readout saying the update had installed. That
  lasted until they dismissed it or restarted the app. A retained automatic failure now clears as soon as a newer
  installed version is seen, and a new unit test covers exactly that case.
- The docs page on updating (Update and uninstall, "Check for an update now") said a check Farhelm makes by itself never
  shows anything unless it installs an update. That is no longer true. The page now says that selecting the version
  number checks for updates, that install failures show at once, and that failures to reach the update server show only
  after three days.
- One SPEC.md sentence listing the ways to check on demand mixed two sentence structures; it now reads as one list.
- SPEC.md now says that a user's failed check which replaced an automatic notice is kept the same way, and that a newer
  installed version clears it. The code and its tests already behaved so.

#### Review notes left as they are

These are minor and were not worth holding the feature for:

- The version's accessible name repeats the whole hover text, so a screen reader announces a long name. A short name
  with the hover text as a description would read better.
- Hovering or focusing the version takes the amber colour off the version text while the must-be-reinstalled warning is
  showing. The warning mark itself stays amber.
- The three-day notice for an unreachable update server can arrive a day late in a rare timing case. That happens when
  the first failure took the full probe timeout and the third-day failure is fast. It is consistent with "at least three
  days".
- An undecodable value in the new state-file field makes the whole state file unreadable. That is the file's existing,
  documented policy, and a test pins it. Decoding just this field leniently would be cheap insurance.
- The browser test's check that the bundled font has the refresh glyph is weak. The executor's screenshot inspection is
  the real evidence.

#### Checks

- Run now, on the combined tree before the landing fixes:
  - `cargo fmt --all -- --check`;
  - `dprint check` on the spec and TODO files;
  - the changelog format lint;
  - `cargo clippy -p farhelm-ui --all-targets -- -D warnings`;
  - the same with `--features desktop`;
  - `cargo check -p farhelm-desktop`;
  - the full desktop package through the recorder (`cargo nextest run -p farhelm-ui --features desktop`, four slots, no
    retries): 588 passed, none skipped, run `08574082-ba69-4a4c-9585-faed93f172c9`;
  - the asset JavaScript harness: 241 passed.
- Run now, after the landing fixes:
  - formatting;
  - `dprint check` on the edited spec and docs page;
  - desktop all-target Clippy;
  - the desktop updater, state and readout tests through the recorder, new test included: 70 passed (the rest of the
    package was filtered out, not skipped at runtime), run `c6908d04-4ee6-432a-b8c9-473534c7c86c`;
  - the docs website build, including its link checks.
- Reused: the executor's browser proof of the readout's styling and motion on Chromium and WebKit, and its bundled-glyph
  screenshots. Neither the combined tree nor the landing fixes change markup, styles or asset JavaScript.
- Still unverified, as the report says: the installed Mac app itself.
