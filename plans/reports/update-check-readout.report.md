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
