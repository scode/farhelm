## Since the last review

PRs #2747 and #2750 were revised; #2749 keeps the same change. No PR was added or dropped, and none has merged. This
round follows the maintainer's decision to repair the browser failures and show failed or uncertain setup and Update
outcomes on the host icon while Hosts is closed.

The product addition is the host warning. The other repairs update tests for the intended sidebar layout and names,
restore a missing session-start premise in a delete test, and correct SPEC.md's header order. The required code and
visual gates cleared the final changes. Broad browser coverage ran on both engines; one later focus failure was repaired
and checked narrowly, as detailed below. No wholly green final broad command is claimed.

## What this was about

The sidebar follows Steel: its sort order is the list title, a status filter sits below it, and a permanent strip of
host icons replaces the host dropdown. Hosts at the strip's foot opens management in place of the session list. The
existing Steel colors, fonts and controls are reused.

Filters preserve the open session and terminal. Choosing a hidden session through the quick switcher resets the host and
status choices when needed. Sounds follow both filters. Host actions, confirmations and running work retain their
existing owners when switching between Sessions and Hosts.

## Things you should know

When setup or Update fails or becomes uncertain with Hosts closed, an independent amber mark appears on that host's
icon. It stays alongside connection state, the update badge and a running retry's ring. Opening Hosts acknowledges the
mark without deleting the diagnostic. A later successful run of the same action can also clear it; starting a retry, an
unrelated success, Connected or a current version cannot. Hosts never opens by itself. An initial Add whose reply is
lost marks only the matching registered host; if none has appeared, the existing Add notice remains. A destination or
installation change that leaves an Update uncertain warns the same logical host entry.

Most landing failures were test premises left behind by the intentional design change: capitalized New and Templates
names, host controls now inside Hosts, session controls hidden while Hosts is shown, footer controls measured at their
old positions, and a terminal-width fixture that needed a wider window after the sidebar grew. Those repairs preserve
their behavior, geometry, focus and request assertions. The header-specific test corrections now belong to #2747, so
that PR is checked independently of later host-panel changes. SPEC.md now states Trash, Templates, New in that order.

The retained delete-refusal failure happened before the browser opened: the session was stopped before its agent was
known to have started, then reported a startup error instead of exited. The test now establishes a live agent before
stopping it and retains the full status if that premise fails. The old evidence cannot distinguish an early-stop race
from the systemd user manager refusing the scope; no product startup change is claimed.

The retained WebKit late-Settings-refusal case exhausted its overall time budget during its last checks. Its trace shows
the refusal arriving, the lower menu moving and closing, and the menu reopening correctly; some final displacement
probes completed after the timeout. It passed the later focused selection without a product or timeout change. The old
multi-host failure left an invented unreachable host registered when its page timed out; contamination of later tests is
plausible, but individual downstream causes were not proved.

The new full browser run exposed ten remaining fixture failures. Two assumed a particular remembered sort title. Seven
WebKit checks searched physical terminal rows for fresh output that actually crossed row boundaries, including the
wrapped-link test's marker before any resize. They now use shorter fresh markers while retaining real output, resize,
wrap and full-link activation assertions. A busy Templates test intermittently saw no scroll from PageDown immediately
after Home reached zero; it now proves a positive native offset and checks keyboard Home returning the real region to
zero, without an immediate reverse gesture. The exact timing cause remains a hypothesis. All twenty repaired cases
passed on both engines; the Chromium Templates reproduction also passed ten consecutive attempts. These are test
corrections, with no product terminal or keyboard change. The latent scroll observation and repair are recorded in
FLAKES.md.

The second complete-coverage batch then found one WebKit Templates focus failure. Save-and-leave publishes the new
template before its scheduled focus handoff completes. The test now waits for the intended name-field focus before
moving to the button whose Enter action it tests. The unmodified case failed three of ten exact attempts; the corrected
case passed ten WebKit attempts and a strict check on both engines. The race's exact native timing remains a hypothesis.
Enter, unsaved-draft, stored-data and close-focus assertions remain. This is another fixture repair, recorded in
FLAKES.md, with no product keyboard change.

The host strip is 56 pixels wide. Sidebar limits are 296–656 pixels, default 396, preserving the session column's old
240–600 pixels and default 340. Stored widths keep their old meaning. At the minimum, header groups can wrap while
remaining reachable. Running host progress occupies its own line. Hosts retains the chosen filter, sort, session and
details setting, and its toggle retains keyboard focus. Host icons, All and accepted session navigation return to
Sessions; a refused handoff keeps the current view. Context-menu opening does not select that host.

## Open questions and possible follow-ups

The remaining Steel plans own the session toolbar, dialogs and inherited sentence case. Structural host reordering can
lose focus on a moved icon; this work does not add arbitrary focus retention for that operation. The prior visual review
left two optional spacing nits: a small running-host name offset and a wide Details gap at the maximum width. Both
remain readable and reachable.

Normal web captures cover the forced macOS title-band layout. Native traffic lights and AppKit drag/zoom are assessed
from code, without a Mac runtime claim. Setup and warning pictures use the existing simulated action backend and real
helm progress. Warning and update-badge coexistence was assessed from their separate CSS corners, not from a captured
simultaneous state. Inherited text-size-control clipping at a 650-pixel main pane belongs to the session-toolbar plan.
No live Farhelm installation or SSH configuration was changed.

## The PRs

- [#2747](https://github.com/scode/farhelm/pull/2747/changes): sort title, count and compact icon actions.
- [#2749](https://github.com/scode/farhelm/pull/2749/changes): status filtering and quick-switcher behavior.
- [#2750](https://github.com/scode/farhelm/pull/2750/changes): host strip, management panel, widths, title band and host
  warnings.

All remain drafts. Final heads are respectively `9b2c8cdc72d9c7f435fcf05523922a47d552bd80`,
`98aa71a3c6db62f8305b6ba8c7a064490c09165c` and `6d94cc8b44143450bc7871090fea7f5bf4a74382`, in one linear stack on main
`98bb15850f5f5ba1df6ed66873338261c45ff948`. The landing monitor owns their merge.

## Checks run, reused and skipped

Every runtime command uses the recorder. Strict browser checks use Chromium and WebKit, three workers with one owned
stack each, pinned tmux and zero retries; generic single-engine repetitions are identified separately. Final folding
preserved the reviewed and tested file contents exactly. The rebase adds only four unrelated trailing-review
instruction/checkpoint changes; it changes no product, test, specification or goal behavior. This comparison supports
reuse of the individual successful commands below, not treating failed commands as passes.

`cargo nextest run -p farhelm-ui` passed 801 tests in `9b42a126-d1d5-444b-85d4-bc83f01ae89e`, complete source
fingerprint `1a72aa15370039ceecfb943dc8c107230bd4c98a7b1490d7b9beb169482136b2`. After the final recovery-order
correction, `cargo nextest run -p farhelm-ui --lib -E 'test(/hosts::attention::|hosts::tests::initial_add_/)'` passed
sixteen in `8f398229-8bf6-4f5b-96bf-1146bf5ec3fb`; 789 cases were outside that selection. Reuse of the full run is
limited to its unchanged tests, not an 805-test pass in one command. Final
`cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed in `a589069f-cfb5-4fe2-85aa-903ef31bc418`. Both latest
commands bind `937b582ecde27e091280983144cac450a8af6abe05dcb9ce2d74b3671a31d467`; later CSS and browser-fixture edits
leave Rust behavior unchanged. The first attempt failed compilation on a visibility-handle type and remains retained as
`b1f2b4e3-3c2b-41d5-bae0-3f29b1cc65bb`; it is not passing evidence.

The requested unfiltered `npx playwright test` ran in `63d3564d-44ed-4b38-9b1f-f1c4da69fb7f` and failed. Raw output
reported 2,742 passed, ten failed and fourteen skipped, but its 18.4 MB JSON report exceeded the recorder's 16 MiB
inspection cap. Those are raw, not recorder-validated totals. The whole failed run and traces remain retained.

Final complete coverage uses two complementary file-pattern batches with the unchanged cap, zero retries and no overall
execution deadline:

- `npx playwright test 'tests/[a-r][^/]*\.spec\.ts$'`, `94504147-0bc1-47bd-b14c-1bd1d9d511bf`: 1,339 passed, three
  skipped, zero failures. Chromium: 670 passed and one skipped; WebKit: 669 passed and two skipped. Its complete report
  is 8,383,358 bytes. Two skips require the manual real-Claude credential/network gate; the other is the deliberately
  Chromium-only pixel matrix's WebKit case. No real-Claude coverage is claimed.
- `npx playwright test 'tests/[s-z][^/]*\.spec\.ts$'`, `ea779e79-d750-4bb6-bb4e-9ed32faee243`: failed, with 1,412
  passed, one failed and eleven skipped. Chromium: 711 passed and one skipped; WebKit: 701 passed, one failed and ten
  skipped. The sole failure was the Templates focus premise described above. The complete report is 10,551,100 bytes.
  Skips were two gated real-Claude spawn cases and nine WebKit clipboard cases, which Playwright cannot grant the
  required clipboard permissions. Their retained skip diagnostics were read. No WebKit clipboard coverage is claimed.

Both bind complete source fingerprint `3b3218f3e0097bda3f1eb3f847d24702e45ff2427f9bad470e55769a2de0c93a`, a dirty tree
above `76e935dc7d2c5a1574b49a5a6f44aa35060683a0`, not that commit alone. Discovery proves exactly 2,766 cases, 1,383 per
engine. Both actual completed reports were checked against that list: no overlap, missing or extra case, with matching
case metadata. Both have complete source identity, output EOF, untruncated retained output and completed cleanup. This
proves complete selection coverage, not a passing second batch or a passing unfiltered command. Actual pinned tmux was
3.7c, binary SHA256 `7913713d94756a96d6b6a7b63041d86ecf31fde6d878ad398bbc2f4fe75e8c2a`.

The targeted formerly failing twenty cases passed, ten per engine, in `071eb913-c29e-4240-9342-02aa4304ca18` on source
`3b3218f3`. Generic recorded Chromium repetition `79b2c58e-ebbe-4d87-82d7-456456a559d8` passed ten busy Templates
attempts on source `c70aa64b6b35f086ec3831fc41c9a266402b81eccf0950544b6a2fd89784f902`; generic mode does not provide
strict runner-validated counts. Its preceding `24cba0e9-9ef9-42bf-b98d-60c26d7689db` recorded three failures and seven
passes and remains retained. A mistaken anchored selection found no tests in `5498b274-03c5-4b24-ad77-9fade3f309b9`,
which supplies no assertion evidence.

For the second batch's focus failure, generic
`npx playwright test --project=webkit-templates -g 'unsaved departures support keep editing, discard and save before leaving' --repeat-each=10 --retries=0`
recorded three failures and seven passes in `c631be9f-ffaa-45ac-994c-d91ca6971230`, source `3b3218f3`. The same
selection after adding the navigation-focus premise passed ten in `3904d54b-caf7-4924-a2c1-d417060346eb`, complete
source fingerprint `249c6d1dbb9c9cc657c2b9bfaea6aaf4dcc473042ec7e7647db8ec7d5d1b9b95`. Strict
`npx playwright test 'templates.spec.ts' -g 'unsaved departures support keep editing, discard and save before leaving'`
then passed two cases, one per engine, in `9aa0a9a6-acb7-4050-a140-fed84df3c1b3`, source
`f3e36418995793c9b3980019c863ca04f406de9ec5903b2f331cecebe3119d41`, with complete report, output and cleanup. These
dirty sources differ only by the test premise and ledger text. No full batch was repeated for that four-line fixture
change: the narrow runs cover its keyboard contract. The failed broad command remains failed, and all its artifacts
remain retained.

`dx build --package farhelm-ui --platform web --release` passed in `76e6d0fd-8f0d-41d4-8217-ed3b8337c9ff`, source
`7aa5fa19acbcf9af194723f0b3bfd099389c8ebc2783633e4879c699ba6633d4`. Later fixture and ledger edits do not change the
built UI. Ordinary `scripts/docs-screenshots.sh --only visual-review --no-build` produced twenty warning-state images in
`1cf315b8-1c09-49e4-8749-36427453480b`, source `3b3218f3`, covering both palettes and wide/650-pixel-main-pane states,
including selected and unselected retries. The executor inspected all twenty. These successful runtime commands have
complete source evidence, untruncated retained output, observed EOF and completed cleanup.

The actual header-only boundary passed its web build and 16 button/tooltip checks, eight per engine,
`a6bb63b7-a6d5-4acc-aebd-3093b0f74b36` and `14493625-50ae-4a65-ae13-f6af459349ae`. The repaired seven selector specs
passed 238 cases, `d4e2c7da-2512-4597-aa2d-e68f5fc86051`; the formerly failing multi-host selection passed 22,
`9fdb5fbb-19e6-4125-b501-fd5163f490d8`. Their covered fingerprints were respectively
`91e83c09541f0f4a64cf4aeab7626082e67f59f9895793cb4f7c8d817ec46b45`,
`ef12474cc1220a08e4f71554737e022ac7e2f69ff23ee035c91d3f2ebb60c031` and
`4d27e6043b4eca15f39c87133f131aa8cb589c486cf8b453394b82201fce98ff`. These checks retain their narrower repair and
header-boundary claims; the later broad commands supply integration observations, not a retroactive passing command. A
focused command that passed only twenty of twenty-six cases remains failed, followed by repaired six-case pass
`27380917-5fdb-422e-b009-47e3ea21cc26`. Those two earlier attempts are historical context only and supply no reused
passing evidence in this report; the identified later selector, host and complete-coverage runs describe the subsequent
checks instead.

Reused from the prior execution: the UI JavaScript harness (`node --test` in its test directory) passed 358 cases in
`c030afe6-17d9-4391-acc8-161be1b00cb9`, source `6834c730d792e2a846eccec8bbed327cc33df7c7a13428a76708d99a7cefee2e`; the
later width/token selection passed nineteen in `4f826f1e-d4af-46ba-a975-49eecc1204b9`, source
`761e76a014e23e5912f4dda2880778b4d06ed314c160cdfe3d88cb0734841632`. Those assets and checks did not change in this
returned round. Status integration passed sixteen cases in `72039ee1-912c-43c6-9f65-0beed4156359`, source
`1a18638b6ba3ae32609e9213899130e47811d0ad9f45624df1af14c30b1e3f7b`; status logic is unchanged. Backend
`cargo build -p farhelm` passed in `fb2117d7-29da-4fb9-80ec-c1fbca97fe9e`, source
`5b51d34a804c58a15d1d23efb98f4c0116cc9d140a89f2eef5059058d486f7ab`; no backend code changed afterwards.

Final source checks passed: `cargo fmt --all -- --check`, dprint on changed configured Markdown, changelog-fragment
format, `git diff --check`, and the pinned-interpreter sleep checker (322 files, no missing rationale). The
agent-instructions checker reported zero errors and two pre-existing unrelated citation warnings. Dprint does not format
TypeScript in this checkout, so no TypeScript formatting pass is claimed.

No full workspace/backend, distribution, installer or native desktop runtime battery was run: this changes the UI, not
those implementations, and focused UI state tests, real-helm browser checks and forced title-band checks cover the
changed contracts. Hosted CI is manual, its browser job is disabled, and no CI run was requested. No failed, interrupted
or poorly identified command counts as a pass.

## Review gate's outcome

The exact gpt-6.1-sol/high header review `8b520021-3238-4d6b-953e-84950099e57d` cleared the original header change and
its independently validated supplement. The unchanged status change retains its exact gpt-6.1-sol/high gate
`e6fa8634-6d53-4d68-baa0-6a0ea9883ff5`. The complete host change and subsequent state, paint and fixture deltas cleared
gpt-6.1-sol/high review `c8560aa0-b604-4f9e-b64a-6a1cc80c6564`. Fresh own-model scope review
`8627d1df-7324-410d-ae97-57dc236035d4` accepted the bounded warning state without an additional authority or persistence
system. The final four-line focus fixture and its ledger entry also cleared the same exact code reviewer. A private
replay of the original reviewed host tree plus every cleared delta produced exactly the final pre-rebase tree; after
folding, the only differences are the four unrelated upstream instruction/checkpoint files. The header tree and the host
change's parent were checked against their independently reviewed boundaries too. No substantive unreviewed delta
remains.

Unchanged header, status and host visuals retain exact Opus 5.5/high gates `fe5ebaf6-be97-4e61-bb24-131962727cc8`,
`f6838e47-65fb-4537-9484-087da35e08a1` and `2fce86dc-fe9d-4282-ba00-1667cfaf8f1d`. The warning gate is exact Opus
5.5/high session `509c4971-5568-459e-aa34-6ae6186cc81e`. It found a ring crossing the dots and an unreadable neutral dot
on light selection. Both were corrected; its follow-up inspected all twenty new images plus three rendered references
and reported no remaining owned defect. Actual successful image reads and terminal success were verified. Optional
spacing and later-plan limitations are disclosed above.

### Landing

Landed on the second delivery, with one specification sentence corrected at landing. The sidebar now follows Steel: the
sort order is the list's title, a status filter sits below it, a strip of host icons replaces the host dropdown, and
Hosts at the strip's foot shows host management in place of the session list. As you decided after the first landing
attempt, a host whose setup or update fails or ends uncertain while Hosts is closed gets a warning mark on its icon,
which stays until Hosts is opened or a later run of that action succeeds; Hosts never opens by itself.

- #2747: sort order as the list title, the count, and compact icon actions.
- #2749: status filter and the quick switcher's handling of filtered-out sessions.
- #2750: host strip, Hosts panel, the failed-action warning, new sidebar widths and the title band layout.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping and a trailing-review checkpoint landed since the plan was based. A fresh-context reviewer
checked this delivery against your answer to the first landing attempt and found every instruction done and no test
weakened to pass. The split you asked for: the warning is the only product change; every one of the 131 failures was
repaired in the tests, by updating them for the intended layout and names or adding a missing starting condition, plus
the SPEC.md header sentence. Most have an established cause. A few do not: whether a delete test's session was stopped
too early or refused by the system's service manager, whether a timed-out multi-host test contaminated the later
failures on the same test stack, and the native timing behind two keyboard and focus repairs remain hypotheses. A WebKit
test of a host menu closing on a late Settings refusal timed out once and passed later without any change. One test step
was narrowed: a busy Templates keyboard-scroll test no longer presses PageDown right after Home, because that failed
intermittently for a reason recorded only as a hypothesis; it still checks scrolling and returning to the top.

#### Fixes made at landing

- SPEC_impl.md said every host row in Hosts always shows its connection dot. The plan hides the dot while a setup,
  update or uninstall runs, when the progress line below the host's name takes its place, and the plan's tests already
  expect that. The sentence now says so. Nothing else changed.

#### Things you should know

- Two failures stay out of sight while Hosts is closed, outside what your decision covered: an Add from the strip whose
  reply was lost before any host appeared (there is no icon to mark; its notice is in the Hosts panel), and a failed
  uninstall (deliberately not marked). Both show when Hosts is opened. A possible follow-up if you want them visible
  too.
- The warning is kept by a new UI module of about 760 lines behind one dot: well tested and reviewed, but more code to
  maintain than the dot suggests.
- The docs website's pages and the docs screenshots (first session, manage hosts, session list, add a remote host, send
  feedback) still show the old sidebar; they change with "refresh the docs" and "refresh the docs screenshots".

#### Checks

This round landed only this plan. It ran, on the stack as delivered, before the one-sentence specification fix:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo check -p farhelm-desktop`, the check that every test delay states its reason, `dprint check` across the
  repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two warnings, both
  about citations main already had), the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh` (the desktop app bundles exactly the files the web UI asks for): all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `0bdae104-7c3e-4b03-bf07-53ae478f89da`: all 805 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `fb61449e-3ccc-4b93-aead-ec86730527a2`: all 986 passed. The UI's JavaScript and stylesheet tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `e2e109c1-99b1-466c-80b4-e9609c1e9a45`: 358 passed.
- `npx playwright test` in `e2e/` with the 19 browser spec files this delivery changed since the first landing attempt
  (among them the sidebar, Templates dialog, multi-host, settings, tooltip, notification, dialog and terminal-link
  specs, and the new host warning spec), on Chromium and WebKit, three workers each with its own helm and supervisors,
  no retries, run `329048ca-b554-4f63-9353-233a34c07814`: all 860 passed.
- Reused, with a caveat: no single version of this plan passed every browser test in one run. The spec files above
  passed at the landed tip. Every other spec file last ran in the executor's whole-suite run, split into two batches
  that together hold every test on both engines: run `94504147-0bc1-47bd-b14c-1bd1d9d511bf` passed all 1339 of its
  tests, and run `ea779e79-d750-4bb6-bb4e-9ed32faee243` passed 1412 and failed one WebKit Templates focus test, which
  the executor then fixed in the test and repeated narrowly (it passed again in the run above). Those batches ran on the
  executor's uncommitted working version; per the report, it differs from what landed only in that test fix, its
  flake-log entry and four unrelated review-queue files from a rebase. The monitor could not compare the two byte for
  byte, because the executor's checkout has moved on.
- Skipped: the rest of the Rust workspace (only the UI crate changed, and only the desktop app depends on it), a full
  browser run on the exact tip (the caveat above), the desktop smoke tests (the plan changes the title band's layout in
  the desktop window, which those tests do not inspect) and the native Mac app (not runnable here). The specification
  fix at landing changes one sentence of text and needs no runtime test; `dprint check SPEC_impl.md` passed after it.

The macOS title band, window controls and window dragging were judged from code and a forced web layout only, as the
report says. Nothing else in the executor's report became untrue at landing.
