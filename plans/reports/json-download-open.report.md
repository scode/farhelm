### What this was about

In the desktop app, Cmd-click (Ctrl-click outside macOS) on a file path in a session terminal saved the file and opened
only Markdown or plain text. JSON now opens by default. The desktop settings dialog lets you save a comma-separated list
of file types to open; the helm enforces that list on both the hover hint and the open request. An empty list keeps
downloads but opens nothing. If the saved file's type differs from the type disclosed at hover, the save still succeeds
but the file does not open.

### Things you should know

On 2026-10-18 the maintainer chose a free-form list with no deny-list, a warning for every type outside a small
recognized safe set, and a specific warning for HTML-like types. Any listed type can open. The dialog warns beside each
type outside `md`, `txt`, `json` and `log`, with a separate browser warning for `html`, `htm`, `xhtml` and `svg`. The
warning appears while the draft is typed, before Save. A malformed list is refused with a visible reason and leaves the
current choice in force.

The HTML warning was checked with local-file pages in Linux Chromium and WebKit, plus browser standards. Those pages ran
scripts and sent requests to loopback services on other ports; some cross-site requests carried cookies, though the page
could not read them directly. This does not establish how Safari or Chrome on macOS prompt, or that every cookie or LAN
destination is reachable. The warning uses conditional language where the evidence is conditional.

This change adds a `helm.db` schema migration. Trying a devbuild on the maintainer's Mac would migrate that helm's
database. The settings field and a real Cmd-click were not observed in a running desktop app.

### Open questions and possible follow-ups

No decision is needed now. A desktop devbuild on a disposable helm would provide visual and end-to-end confirmation of
the field and opener; the current checks cover the policy, parser, warnings and desktop compilation but not a running
window.

### PRs

- [#2296: open JSON downloads and enforce the saved list](https://github.com/scode/farhelm/pull/2296/changes) — draft at
  delivery, based on main.
- [#2299: edit file types in desktop settings with live warnings](https://github.com/scode/farhelm/pull/2299/changes) —
  draft at delivery, based on #2296; removes the completed TODO entry.

### Checks run, reused and skipped

- Run: `cargo check -p farhelm-helm`, `cargo check -p farhelm-ui --features desktop`, `cargo check -p farhelm-desktop`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo fmt --all -- --check`, `dprint check`, changelog format, the isolated test-delay check, and the docs website
  build. The website build validated every internal link. The last test-delay check inspected 304 delays and found none
  without a rationale.
- Recorded focused Rust runs: `b566901e-ed4d-44a2-9ada-2fdd6027ea75` passed 11 parser, preferences, download and schema
  cases before the parser's normalized-size fix; `999e8d95-44d1-4c8e-88e6-1419c6604c30` passed five parser cases after
  it. `bab34233-be11-42a7-b77d-7ab560f9158a` passed four desktop warning-list cases after the code review fix.
  `2787259d-f5ea-405a-b59f-c57de5306da5` passed the expanded case with both HTML and another unfamiliar type after
  duplicate entries. All selected cases ran; other nextest cases were filtered out, with no runtime substrate skip in
  retained output.
- Retained first compilations `18d49605-4b93-4443-863e-ed1cb411af05` and `177bff59-fad0-431e-87e1-8c494d128c38` failed
  because old test fixtures omitted the new preference field. Both were fixed in this session before the passing runs;
  neither failure executed a runtime case.
- Reused: recorder `b566901e-ed4d-44a2-9ada-2fdd6027ea75` covered the pre-cap PR #2296 source (source fingerprint
  `f276264cc733` at base `b111e0d8`). Its helm behavior cases still cover unchanged helm code after the parser-cap fix;
  the parser was checked separately afterward. The desktop compile checks covered the pre-review PR #2299 working tree
  on PR #2296 (`e7a33f0e`), before only the warning scan and its tests changed; the later focused desktop test
  compilation covers that final warning helper. Main's later host-picker and launcher-layout fixes touch separate
  sections and do not change that coverage.
- Skipped: a full Rust battery and browser end-to-end suite would not target the changed opener policy or desktop-only
  field more closely than these checks. The browser test stubs the hover's open hint. The live desktop app was not
  launched because this machine's installation is off-limits.

### Review gate

Each draft PR received its one required fresh-context correctness, design and idiom review. PR #2296's reviewer found
that normalization could expand an accepted policy beyond its storage cap; the parser now refuses that case and a
round-trip test covers it. PR #2299's reviewer found that scanning only 32 raw tokens could hide a warning after
duplicates or empty tokens; the scan now reaches all tokens in a valid draft, and the targeted test covers HTML and
another unfamiliar type. No review finding remains open. The second reviewer checked the HTML warning against the
local-file evidence.

### Landing

Landed on its own in round 96, as #2296 and #2299, merged bottom-up, with several fixes made during the landing (below).

#### What landed since the stack was based, and whether it interacted

Since this stack's base, main gained the fix that closes the launcher's host list on an outside click (#2294) and the
fix that fits a filled-in session launcher in a 900-pixel window (#2295), plus a TODO.md entry and plan bookkeeping. The
landing rebase was clean. The landing monitor read the stack against them, and a fresh-context reviewer did the same
independently. Neither found an interaction: this stack's only change to the launcher is a test value that names the new
setting, and its new style rules apply to Settings alone. Opening a downloaded file still requires the file link's hover
to be fully visible and uncovered, with the screen text unchanged since the hover, the helm applies the current list
when the file is opened and requires the saved file to keep the hovered file's type, an empty list opens nothing, and a
malformed or unreadable list opens nothing either. A program in a terminal cannot change the list, because changing it
needs a signed-in device.

#### Upgrade

#2296 moves the helm database from schema 60 to 61; the protocol is unchanged. An older helm refuses the newer database.
As the report says, trying a development build on the maintainer's Mac would migrate that helm's database.

#### Fixes made during the landing

Running the whole helm library and the UI style tests, rather than the executor's selections, found two problems in
tests the executor did not run, and the cross-plan review found two stale texts; all were fixed before merging:

- Two helm tests that rewind a database to an older schema failed because one of the rewinds did not remove the new
  file-types column. The rewind now removes it, and the whole helm library passes. This is the same kind of break that
  reached main twice before, with the tunnel and managed-agent trust settings, and was caught once more at landing.
- The new warning beside a risky file type glowed with a smooth animation that never stops. The app's style tests refuse
  that, because a smooth endless animation once held the Mac's window server at 40–50% CPU even with Farhelm's window
  hidden. The glow now changes in steps, under three times a second. The warning and the file-types field also used
  their own corner rounding instead of the app's standard sizes; they now use those.
- The Settings footer in a browser said that file types use the save button, though only the desktop app shows that
  field; the browser now leaves file types out of that sentence.
- The Session list docs page still said file downloads are unchanged in the section that now leads to the file-types
  choice; it now says the web-link choice does not change file downloads, as the app's own help text does.

#### Possible follow-up

A Cmd-click can open a file whose hover never offered to open it: the app sends the open request on any modifier-click
and the helm checks the list at that moment, so if another signed-in device changes the list between the hover and the
click, a type can open without its hover having shown the open hint, and, if the list was changed through the helm's API
rather than a desktop app's Settings, without anyone having seen the warning for that type. Sending the open request
only when the hover offered opening would close that.

#### Checks

- Run now, on the rebased stack with the landing fixes (the landed diffs are identical to it):
  `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo check -p farhelm-desktop`, the isolated test-sleep check (304 delays, none without a reason); through the test
  recorder with pinned nextest and tmux, four slots and no retries, the whole helm and UI libraries: 1730 passed, run
  `d69ab2f2-b47c-443a-a0d7-8cea85eef1b3`, and the desktop UI targets: 689 passed, run
  `41bc5719-b788-4829-9d24-3436ebcd23c8`; the UI JavaScript harness: 294 passed, run
  `e39a7bcf-e73b-47f9-8a30-b4db41375609`; `bun run build` for the docs site (32 pages, internal links checked); then a
  fresh web build and
  `npx playwright test settings.spec.ts terminal-link-clicks.spec.ts terminal-links.spec.ts host-picker.spec.ts launcher-enter.spec.ts`
  on both engines, one worker, no retries: 138 passed, run `341385a6-9324-4083-9403-bba4a584c78b`.
- Retained earlier observations: the first whole-library run, which also covered the protocol library and the `farhelm`
  binary (2077 passed, the two rewind tests failed), run `5fac4596-42df-4880-8d5f-f34d8dfc5f28` (the landing fixes touch
  neither the protocol library nor the binary, so that run's passes for them still apply), and the JavaScript run with
  the two style failures, run `370c7bb7-ddf6-40ed-b3e2-26b82438d6ca`.
- Skipped: a running desktop window, as in the report; this machine's own Farhelm installation is off-limits.

Nothing in the executor's report became untrue at landing, apart from the checks and fixes above.
