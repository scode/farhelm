# Managed checkout trash

## What this was about

A GitHub checkout Farhelm created looked much like an ordinary working directory, even though deleting its final session
moved the entire checkout into an archive. The archive preserved uncommitted work but did not show users where it went
or how to clean it up. The maintainer chose to keep archiving, call these directories managed checkouts, mark their
sessions, expose the destination choice, and put a trash beside New.

The draft stack implements that design. Managed checkouts have branch marks and repository names in the session list and
session view, including sessions borrowing another session's checkout. The launcher and template editor offer folder or
managed checkout explicitly. The trash shows recorded archives by host, newest first, with archive time, allocated disk
usage and the directory to copy recovery work from. It offers confirmed permanent cleanup on one host or across
reachable hosts. Both covered TODO entries are removed.

## Things you should know

Archiving still preserves the entire checkout and frees no disk space. Permanent cleanup is a separate confirmed action;
there is no restore action, per-entry deletion, automatic cleanup or new count timer. The count follows loads,
connection changes, confirmed local archives, managed-session disappearances between complete listings, dialog opens and
cleanup. A different window emptying the trash can leave this window's count high until its dialog opens.

Unreachable hosts show unknown counts and cannot be cleaned. Global cleanup waits for every reachable host's successful
listing, so a failed or pending read cannot silently remove a host from its confirmation. The confirmation freezes its
selection: an archive arriving afterwards is not added. Diagnostics can remain after cleanup, including several for one
checkout, and unreadable cleanup replies warn that some contents may already be gone.

Permanent removal is limited to recorded retired archives, with root and archive identity checked again. A replaced root
or archive is refused at admission; traversal does not follow symlink targets or enter unrecorded neighbouring trees.
Working directories of retained sessions, stopped sessions included, prevent cleanup. An unusable ownership record may
be forgotten on confirmed cleanup, with its preserved path disclosed. Passive reads discard only positively missing
archives under a verified root. Concurrent replacement after admission has the limits described below.

Safe recursive cleanup required more engineering than the outline's path-based removal suggested. An independent scope
review accepted keeping open handles to verified directories while traversing them, rather than repeatedly resolving
filesystem names. Cleanup refuses symlink and mount traversal, caps depth at 32 and bounds repeated scans of a changing
directory. It can partially remove an archive before failing, and reports that outcome. While a cleanup batch runs,
launches, restarts and session Deletes that need the same host's directory lock wait for it; a large batch can delay
those actions.

Moving a directory out of the archive after cleanup has opened it does not save its contents: cleanup continues through
the verified open handle, even under its new name. The final removal uses the original name and can remove an empty
replacement directory at that name, but cannot recursively delete a nonempty replacement's contents. A concurrent rename
can therefore leave partial cleanup or an error, rather than cancelling deletion. Copy recovery work out before
confirming cleanup; moving directories during cleanup is not a way to cancel it.

Size measurement shares a two-second, 100,000-entry budget per host, counts allocated blocks, and deduplicates
hardlinks. Incomplete measurements show unknown sizes and partial totals. Archive dates come from the recorded rename
timestamp; historical names without a valid timestamp stay unknown. Linux without the required confinement support can
list identity-verified archives but cannot measure or recursively remove them. The protocol is now 44; internal checkout
names and stored template JSON remain unchanged.

The header keeps Templates. At the 240-pixel sidebar minimum the trailing actions wrap together instead of clipping; the
trash stays immediately left of New at the same height. Only this window's affirmative archive reply plays the finite
flight and wiggle, and reduced motion skips both. The desktop-only unit fixture also needed the host appearance defaults
introduced by earlier work; that compile repair is included.

## Open questions and possible follow-ups

No maintainer decision is needed to deliver this stack. macOS descriptor behavior has not been exercised on macOS in
this run; the desktop compile and WebKit browser checks cover different layers and do not establish that filesystem
runtime proof. Very deep trees or hosts above the 10,000-entry cleanup batch limit remain visible with a refusal rather
than being silently omitted. Changing those limits or adding restore, individual cleanup or unreachable-host caching
would be separate work. The pre-existing borrower-lifetime end-to-end timeout remains open in the Deflake bucket; its
recurrence during final validation is disclosed below. No product defect was established from that timing failure. The
existing cleanup cascade also remains open: a session-cleanup error before fixture close can skip Git configuration
restoration.

## The PRs

- [#1785](https://github.com/scode/farhelm/pull/1785/changes): managed-checkout naming and session marks.
- [#1786](https://github.com/scode/farhelm/pull/1786/changes): explicit destination choice in launcher and templates.
- [#1787](https://github.com/scode/farhelm/pull/1787/changes): verified host-owned trash listings and permanent cleanup.
- [#1788](https://github.com/scode/farhelm/pull/1788/changes): trash dialog, confirmations, counts and local archive
  cue.

The stack remains draft. No release, deployment or merge was requested or performed by this executor.

## Checks run, reused and skipped

Final-source checks passed: Rust formatting; changed Markdown formatting; changelog format; the isolated source-only
sleep check (278 delays, zero unexplained); and whitespace checks. UI desktop compilation and all-target desktop-feature
Clippy passed, as did the release web build and the docs website's frozen install/build (32 pages and internal links).
These cover the shipped UI, desktop-only compilation and changed product documentation. No CSS or TypeScript formatter
pass is claimed: the configured formatter does not handle those files.

Recorded Rust checks used the owned Linux sandbox, pinned nextest and pinned tmux, four slots and zero retries:

- Backend run `2a987b2c-becc-479a-802f-87854590a089` passed 29 selected protocol, archive outcome, session lifecycle,
  cancellation and trash tests. Corrected run `b85c508c-7e16-4779-849d-88aeb8f4464f` passed 16 trash and helm-route
  cases after review changes; final run `93730ab2-7d73-4323-960a-5abdeac7c403` passed the exact malformed-ID route. Real
  mount-boundary tests ran. Backend all-target Clippy and the shipped CLI's feature configuration passed.
- UI run `d4654731-8651-4933-a5ae-1d57838b178b` passed three selected tests: frozen/deduplicated cleanup selection,
  unreadable Delete reply disclosure, and the repaired desktop startup fixture. Later copy, CSS and equivalent lint
  rewrites do not change those contracts, so this evidence is reused. No selected backend or final UI runtime substrate
  skip was reported.

Browser checks ran through the recorder on Chromium and WebKit, using the matching release web builds and one worker:

- PR1 run `ec6e9657-b9b0-4ec9-bc47-3eab39290f0b` passed 50 selected naming, marks, tooltip and Delete scenarios. PR2 run
  `e7eaa4e7-27df-44a9-a1f0-86e7c9c49201` passed 20 final destination/template/tooltip scenarios;
  `7731f9a4-496f-4aa7-8a75-4dde3eed63de` supplies 54-case coverage of unchanged checkout allocation and retry paths.
- PR4 exact cue run `26cb3b45-3eeb-491d-afa3-842c8fba3514` and exact global-cleanup run
  `19e6312d-7804-4ffa-8a1b-00f266b5794f` each passed two cases, one per engine. Final affected-file run
  `6a629bb4-ed2b-4d80-9a8e-c95f71f0d192` ran all checkout and sidebar-resize cases: Chromium passed 14 of 14; WebKit
  passed 13 of 14 and the existing borrower-lifetime case timed out at its 60-second total limit. All four new trash
  cases and all four sidebar cases passed. No case was skipped or left unstarted. The exact fresh-stack WebKit
  reproduction `7510f5f0-49da-4204-aba6-13a6aee817e0` also timed out (one of one). Traces show forward progress rather
  than a failing semantic assertion. This test already has a clean-main timeout history and an open Deflake TODO; the
  new recurrence is recorded in FLAKES.md. Cause remains unknown and the deadline was not changed. These runs are failed
  evidence, not passes. An earlier title-selector attempt found no tests and adds no runtime coverage.

Visual inspection covered the launcher, template editor, row marks, trash dialog and confirmations against the approved
mockup. Final 340- and 240-pixel sidebar captures were inspected on both engines: the controls fit, the trash and New
have equal heights and stay adjacent, and the hosts header retains its earlier styling.

Earlier same-session compile and fixture failures were retained privately, corrected and followed by focused passing
runs; they were not treated as latent flakes or erased by later passes. The archive-rename permission-refusal test could
not exercise its permission premise because the sandbox runs as root. No later run exercised it as an unprivileged
process, so that runtime permission-refusal coverage remains unverified.

No full workspace or browser battery was run: the selected safety, lifecycle, route, launcher and trash scenarios cover
the affected risks. No asset-JS harness or asset-table check was needed because no asset file or standalone JavaScript
asset changed. Installer, provisioning, release and deployment checks were skipped because those behaviors did not
change. macOS filesystem execution remains the gap stated above.

The careful rebase reviewed every intervening main diff. Remote uninstall now ends its private tmux server; its
provisioning paths, confirmation and tests are independent of checkout archiving. The shared supervisor teardown edit
only documents that uninstall exception, and both independent spec sections are preserved. No runtime uninstall rerun
was needed for this stack.

## Review gate outcome

Each code PR received the prescribed fresh Opus 5.5 review at high effort, with the full test-authoring checklist.
Concrete findings were checked against the source and corrected locally. Independent scope reviews examined the safety
redesign and whether the UI corrections added unnecessary machinery. The backend confinement redesign and final UI shape
passed those reviews, including the decision to keep the fleet-wide read generation. No blocking finding remains. The
later FLAKES.md addition records the existing timeout without changing tested behavior. Commit and PR wording passed
fresh cold reads.

Implementation and investigation were local; only prescribed reviews and cold reads used agents. Opus runtime identity
and structured usage were recorded. Native-reader and orchestrator usage are unavailable. Private review and usage
records remain retained alongside the test evidence.
