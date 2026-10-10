## What this was about

Every remote host used the same cloud, making sessions on different machines hard to distinguish. Remote hosts now offer
cloud, house, flask, database, chip, rocket, gear, gem, hexagon, triangle, ring, square, bug, factory and castle. The
choice is shared by every client connected to the helm and appears in session rows, host rows, the quick switcher and
the launcher's host picker. Host settings also has the approved grouped layout, while keeping immediate saves and the
existing destination, alias and permission controls.

## Things you should know

The local host keeps its red laptop and has no Appearance section. An untouched remote host keeps the default cloud and
text color. Only the icon carries the chosen color; names, rows and terminal content keep their ordinary colors. The
colors are default (the existing text color), lavender `#b8a4f0`, orchid `#e090d8`, teal `#4fc8c0`, steel `#93aec4`,
sand `#c8b48e` and copper `#d0906a`. All six requested hues passed the contrast checks without adjustment, and none of
the supplied icon geometry changed. The asset harness requires at least 4.5:1 for each hue against the sidebar/dialog
surface, raised menu surface, selected row, selected row with its menu open, hovered/focused control fill and input
well. Those bindings cover the colored icon's resting, selected and browsing surfaces in the session list, settings,
launcher picker and quick switcher; these are calculations for opaque token colors. Opacity-reduced disabled controls
and screenshot pixels were not measured.

The choice is stored on the helm as stable words, such as `rocket` and `teal`. The helm saves the two words together,
rejects unknown words and local-host changes, and notifies open clients only when the choice changes. The database
advances from schema 43 to 44; an older helm cannot open it after upgrade. The targeted migration tests check that
existing hosts keep their defaults and settings and that the migrated schema matches a fresh one. This is schema
evidence, not a full installed release-upgrade test.

The launcher's picker keeps focus on its named host control. Arrows, Home and End browse; Enter or Space chooses an open
option; Escape discards browsing. Typing finds a host name, with repeated letters cycling and a pause starting a new
prefix. Enter on the closed control retains the existing Launch action. The entire launch round trip disables the
picker, and changing the host retains the existing rules for saved folders, browsing, clones and launch retries.
Choosing the same host does not reset the draft. The template editor retains its native host field.

The settings refresh uses host-specific styles, so shared dialog styles and other dialogs keep their layout. Destination
and alias edits still exclude other edits, saves retain their individual outcome messages, and a short window can scroll
to the close button. A newly saved appearance is published before a refresh can overwrite it with an older pair;
changing the color while an earlier icon refresh is pending therefore preserves the icon.

Screenshots were inspected on Chromium and WebKit: the local dialog retained its red laptop and omitted appearance
controls; the remote dialog showed fifteen icons in an eight/seven grid, a selected teal swatch, and a matching rocket
in its header and preview. The chosen rocket was teal in the host row and selected session row. The open launcher menu
and switcher showed the same teal remote rocket beside ordinary text, with a red laptop for local. Long destinations
wrapped, and the short settings dialog's footer remained reachable.

The stack was carefully rebased over sidebar resizing, compact approval cards and timer-owned supervisor refreshes. The
only textual conflict was at the start of the CSS token block; both the new sidebar width and host colors were retained.
The host storage and appearance route do not overlap those changes. Combined browser checks exercise the launcher
against the new layout and running supervisor timer.

## Open questions and possible follow-ups

No decision is needed to complete this plan. The requested icon/color TODO entry is removed. Screenshot regeneration was
expressly out of scope: `manage-hosts/host-settings` and `start-a-session/host-yolo-setting` still show the older
settings layout, and launcher shots such as `start-a-session/destination` still show the native host field. Their
capture driver now understands the custom picker, but the published images have not been refreshed.

## The PRs

- [#1765](https://github.com/scode/farhelm/pull/1765/changes): organize host settings into sections.
- [#1770](https://github.com/scode/farhelm/pull/1770/changes): choose remote host icons and colors.
- [#1775](https://github.com/scode/farhelm/pull/1775/changes): show host icons in launcher and quick switcher.

They form one linear stack and remain open drafts. None was marked ready or merged by the executor.

## Checks run, reused and skipped

On the combined stack rebased over the new sidebar and supervisor behavior, `cargo build`,
`cargo check -p farhelm-ui --features desktop`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, and
`cargo clippy -p farhelm-helm -p farhelm-proto --all-targets -- -D warnings` passed. The web release bundle built
successfully. These checks establish native/desktop compilation and matching browser assets; none is a desktop shell
runtime test.

`cargo fmt --all -- --check`, `dprint check`, the changelog format check and the isolated test-delay checker passed
before the final fixture-only correction. The delay checker inspected 276 files with zero unannotated delays. After that
correction, `dprint check`, the changelog check and the same isolated delay checker passed again; Rust formatting was
reused because no Rust source changed.

Recorded combined browser run `219019be-3701-4a7f-a6e6-23e9bed45091` executed 66 selected cases: Chromium 33 passed;
WebKit 32 passed and one failed, with no skips or retries. The selection covered four picker tests, saved-destination
authority, stale callbacks, launcher Enter and Tab behavior, host aliases, remote browsing/reset/clone handoff, checkout
previews, disappearing and unreachable hosts, busy state and retry intent. The failure was the existing key-minting
destination test's ungreeted feed closing on the ten-second silence deadline. Its retained timeline establishes that
lifetime boundary; three exact unchanged WebKit repetitions in generic recorded run
`78f7a8c1-1d1d-47ae-bce7-b32191c18043` passed, which does not erase the failure. Both destination fixtures now greet the
feed as the real helm does and check for a live peer before notifying, preserving the held key-creation race. The
corrected destination spec run `e3b9f09e-c985-42b5-87b5-a080c01a44a0` passed all 14 cases on both engines with no skips
or retries; the other 52 combined-run cases retain their coverage because only the destination fixture and its failure
record changed. The latent failure is recorded in FLAKES.md.

Reused evidence remains applicable because the rebase changed no appearance store, route, migration or tested pure asset
contract: appearance/migration run `5d9b28d9-5355-4881-bcf5-3ec06d78b135` passed five targeted Rust tests; JS asset run
`53f329f6-1e2b-419b-8108-9e3547728dba` passed 203 tests; the website built 32 pages and checked internal links; settings
browser run `8f894a82-1827-4e61-8ccf-966753e1c754` passed 12 cases. Initial picker run
`41659a38-b37a-4091-8777-55a4bbaed330` passed eight cases on both engines, including the quick switcher's chosen host
mark, which the final 66-case regex did not select. Source review corrected unmatched typeahead afterwards; that
interaction was retested in the final selection, while the switcher mark code remained unchanged.

The earlier evidence is retained, including failures; a corrected pass does not replace a failed observation. The
settings/appearance browser run `8be9e579-1ae4-4f3d-aff2-682f68bd8116` passed 22 cases and failed the new two-client
case on both engines because both clients attached the same terminal and the last attachment displaced the first. That
assertion only needed the selected row, so its unrelated terminal readiness requirement was replaced with the explicit
selected-row state. The corrected narrow run `4fbdca9d-d4d8-4835-9768-21f2e89d9b31` passed both engines; the other 22
passes remain applicable.

The new picker correction run `1b8eb5c9-ee1d-4600-a867-9ced815e3e18` passed three cases and failed WebKit teardown after
all picker assertions had passed: an intercepted host refresh raced disposal of its response. The spec now uses the
existing shared hook that drains active routes before disposal. Exact corrected run
`4977e1d3-0b37-4e5e-b895-00ddec9c2383` passed both engines. An earlier JS run also exposed a new literal switch radius;
it was replaced by the existing token, with the focused token checks passing. These were same-session fixture/style
defects and were not added to the latent-flake log. Recorder refusals before execution and the interrupted web build are
not counted as test passes.

Full workspace, desktop runtime, installer, release, provisioning and full browser batteries were skipped because the
changes introduce host presentation metadata and a bounded launcher control; targeted route/migration, asset and real
browser evidence cover those contracts. No live installation was changed, and no website deployment was requested.

## Review gate outcome

Each code PR passed the required independent correctness, design and idiom review, requested on native GPT-6.1-sol at
high effort, with the full plan charter and test-authoring checklist. The settings review strengthened a visibility
premise. The appearance review found and closed the stale refresh race, historical migration-fixture column ordering,
and an SVG tooltip target issue. The launcher review found and closed unmatched typeahead opening at an old browsing
index instead of the committed host; regression tests cover a remote default and a later changed host. Its teardown and
healthy-feed fixture corrections also passed review; the final reviewer independently checked the retained feed timeline
and the cause wording in the latent-flake entry. No source findings remain open.

Commit/PR wording passed fresh cold reads, requested on GPT-6.1-sol at medium effort; bodies are empty because the
titles and diffs carry the changes without extra motivation to preserve. Implementation and investigation stayed with
the executor, as required by no-workhorse mode. Reviewer claims were checked against source and the executor's recorded
validation. Exact native runtime model attribution and usage counters were unavailable; requested model names are not
proof of actual attribution. Private orchestration evidence remains under session UUID
`a63f6b2c-45f7-4830-bf84-ea76de63472d`.

### Landing

Landed on 2026-10-10 (UTC) as three squash commits on main, in order: #1765 (host settings organized into sections),
#1770 (remote host icons and colors) and #1775 (host icons in the launcher and quick switcher). Since the stack was
based, main gained the shutdown quiet-down fix (#1780, supervisor only) and plan and triage bookkeeping; the rebase was
clean. claude-compaction-status landed directly after it. Other plans reviewed in the same round
(terminal-file-download, waiting-sound, hook-report-watch, omp-pane-guard-all-launches) did not land with it; the
reviewer found no overlap with them beyond separate hunks of shared files, and none touches the host rows or the
launcher's host control.

#### Review before merging

A separate reviewer that had not worked on this round's plans read the stack against main and the plans landing near it.
It found no path from a host's stored icon or color into markup or styles other than the fixed word lists, the helm
route refusing unknown or extra fields and the local host, the database migration and its downgrade fixtures consistent,
the new layout free of any assumption about the sidebar's old fixed width, and the many edits to other browser specs
straight swaps to the new host picker rather than weakened assertions.

#### Fixes made while landing

- The launcher's new host menu relied on its button having keyboard focus: it closes when focus leaves and takes its
  arrow keys, letter search and Escape on the button. Safari and the macOS desktop app do not focus a button when it is
  clicked, so a menu opened with the mouse there would not close on a click elsewhere and would ignore the keyboard.
  Opening the menu now focuses the button itself. A browser test opens the menu without giving it focus (as those
  engines do) and checks that it takes focus, answers the arrow keys, and closes on an outside click. This was in #1775
  before it merged.
- Every icon and color word is written three times in the code (for the web API, for saving and for loading), and one
  stored word the helm cannot read would make the whole host list fail. Tests now check that every word round-trips
  through all three, and the code says that adding a word needs a database version bump. In #1770.
- Two existing helm tests failed with the stack: a fixture that rebuilds an older database to compare with a frozen copy
  needed the two new host columns removed (as the executor had already done for the store's own fixtures), and the
  shared file that pins the helm's host-list response needed the new icon and color fields. Both fixed in #1770; the
  executor's runs had covered only five targeted helm tests.
- A stylesheet comment still said the local laptop's red was limited to session rows. It now says what the code does,
  below. In #1770.

#### Not changed while landing

The local laptop mark is now red everywhere it is drawn, the hosts panel included; before this plan its red was limited
to session rows on purpose. Your decision said the local host "keeps its red laptop", and the launcher and switcher
showing it red follows from that, but the hosts panel change was not mentioned anywhere. Whether the hosts panel should
show it red is yours to confirm; changing it back is a small style change.

Smaller notes left as they are: a color click resends the client's current icon with the new color, so a near
simultaneous icon change from another client can be overwritten (last write wins for the pair); the letter-search test
cannot tell multi-letter from single-letter matching; and a few stale names of the old remote mark remain in comments.

#### Checks

- Run now, on host-icons and claude-compaction-status stacked together: `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, and `cargo fmt`.
- The protocol, helm and UI unit tests in full, with the supervisor's screen tests: the first run (`aca47509`) found the
  three helm test failures above; after the fixes, the protocol, helm and UI tests passed 1592 of 1592 (run `9e9c2d1e`).
- Through the recorder on Chromium and WebKit with pinned tmux 3.7c, one worker and no retries: the host picker, quick
  switcher, sidebar, multi-host terminal, settings and tooltip specs (run `865e8fca`, 344 of 346; the two failures were
  the first version of the landing's own new host-picker test, which clicked a field the open menu covers), then the
  host-picker spec again with that test corrected (run `9592311e`, 12 of 12).
- Reused from the executor: the destination-authority, clone, launcher and checkout specs it ran on the combined stack,
  the UI JavaScript tests, the website build and the changelog lint. The landing changed none of what those cover beyond
  the host picker, which was re-run.

Nothing in the report above was made untrue by the landing, except that the hosts panel's red laptop, which the report
does not mention, is a change.
