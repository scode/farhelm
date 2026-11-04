# Launch progress

## What this was about

A slow host left the session launcher looking stuck. After the app's one-minute request deadline, it could say “Not
launched.” even though the helm was still completing the launch. The same silence affected Restart and Replace.

The launcher now shows the host and elapsed seconds after two seconds, beside its disabled buttons. Quick launches and
template shortcuts show the same status in a bottom pill. Plain Restart and all three plain Replace surfaces show the
wait too. Fast replies look as before, and the status disappears when the request settles. Assistive technology gets one
announcement rather than an announcement every second.

## Things you should know

The agreed scope stays in the GUI. There are no new server messages, launch steps, retries or cancellation controls. The
existing one-minute deadline and retry identities are unchanged. The displayed wait begins when the prepared launch
enters the API call; it can include waiting for preceding configuration saves.

Deadline expiry now says “Still waiting on [host]” and explains that the operation may still finish. New and Clone
retain the same-settings guarantee against a second copy. Replace makes that promise only while its original retry
identity remains: cancelling, changing settings or starting another Replace ends the guarantee. Restart and Restart with
ask the user to check the session before restarting again, because neither has a retry key. Actual helm refusals retain
their old wording.

Two implementation constraints needed adjustment. Browser engines discard the transport's typed timeout evidence
differently, including while reading a successful response's body. A request-owned browser marker now records the
existing deadline firing; it does not abort, retry or settle the request. The open session's header cannot fit a status
inside its fixed-width controls at a 650-pixel pane, so its status appears immediately below that toolbar. Before two
seconds it adds no height. The interrupted-session card and sidebar keep their status beside their actions.

The changed documentation page is `website/src/content/docs/docs/using/start-a-session.mdx`. Creation, Restart and
Replace behavior is recorded in the specs, the launch-feedback TODO entry is removed, and one changelog fragment covers
both PRs.

## Open questions and possible follow-ups

A dropped connection that is not a deadline expiry still uses the old “Not launched.” wording. That known limitation was
expressly outside this plan; it can still misdescribe an unknown outcome.

An additional host-terminal regression run stopped before any test because the harness checked for its startup session
while that session was still being created. The failed run is retained as `1287cb5b-7bcd-4ba4-9628-87100731c260`. A
narrow startup observation passed afterwards; this does not establish the failure's frequency or eliminate the harness
readiness risk. No product behavior was changed to work around it.

## The PRs

- [#2970: pending session launches](https://github.com/scode/farhelm/pull/2970/changes), draft,
  `c3387315057902f7798305da4fe2f4d3bce405a9`, based on main.
- [#2974: pending Restart and Replace](https://github.com/scode/farhelm/pull/2974/changes), draft,
  `bc9625b7a8cc75c0a16d4876c38f50ee45f767eb`, based on #2970.

## Checks run, reused and skipped

The final executable code passed the UI unit suite through the recorder: 842 tests, run
`3b6eccaa-7351-44cf-80e9-dd35020d6499`, using `cargo nextest run -p farhelm-ui`. UI test compilation,
`cargo check -p farhelm-ui --features desktop`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, and the
release web build passed. Later edits only clarified comments and specification wording; they change no executable code,
doctests or generated artifacts, so those builds and tests remain applicable to the published heads.

These checks ran before the final changes were committed. Their working tree was based on
`53cdd2bad1e4a4d5b62c2bdc7dfe1cda34b9797d`, with the corrected deadline marker and second PR already present as local
changes; that old commit alone is not the tested source. The recorder identifies the unit-test tree by fingerprint
`d9cacc5ac21beed5468c1c57e3812028f8526fb818738bc55b4289e5304edd9c`, the full new browser run by
`48800614f2d746f643db86974f86770c931637bc6a99085a4bed59f7319f59ec`, and the host-new/default-existing runs by
`0d7cccb5b496c9a0b539e5785cc78ba14e2bcfb6d29d99ae5abba524807a9d87`. Only comments and specification wording differ
between these captured trees and the published executable code. The final host-existing run used the clean published tip
`bc9625b7a8cc75c0a16d4876c38f50ee45f767eb`.

Chromium and WebKit each passed all fourteen new browser cases in `launch-progress.spec.ts` and
`restart-replace-progress.spec.ts`: 28 executions in `5b9a33cf-751d-44ac-9876-527c1135fd1d`. The host-terminal variant
passed the same 28 executions in `62896857-45ff-41ff-971c-70ad8ba44481`. These cover the threshold and counter, host
name, quick-launch and template pills, accessibility, actual deadline expiry before headers and during a held successful
body, a timeout-worded server refusal, all Replace surfaces, retained retry identity, Restart with, and header geometry.
The two deadline cases also passed separately on both engines in `aa19d8b4-1ddf-48f2-9015-af3c874cd5b2`.

Six existing create/retry, disabled-input, refusal and Restart/Replace cases passed on both engines: twelve executions
in `2841077e-b284-4d1f-9268-c84f48e8e189`. The startup-only failure above ran none of those host-variant cases; the
narrow Chromium retry observation passed one case in `7b178b6e-d28b-4176-9d22-75306c368c44`, then all twelve
host-variant executions passed in `71c9ad2b-fff3-4fe8-901d-0b0de1b21484`. Successful recorded runs were checked for
complete reports, retained output through EOF, runtime skips and cleanup limitations.

Development failures are retained separately: `b79c7ea8-16b5-4a68-a960-0dd9328a1f58` reproduced the old response-body
timeout bug; `4f39d81c-c91e-4aa4-91bf-537e0d3907c1` and diagnostic `25de0895-afb5-474d-9ea2-ec7b1ec36f5a` exposed
browser error-shape differences and a test-helper capitalization error. The final marker and corrected tests passed
afterwards. These same-session failures are not counted as passes or latent flakes.

`cargo fmt --all -- --check`, Markdown formatting on the changed pages/specs, changelog format validation, and the
isolated test-sleep checker passed (397 delays inspected, none missing a rationale). The phone footer and 650-pixel
header screenshots were inspected. WebKit and the native-tunnel CSS preview supply engine and geometry evidence, not a
macOS native-app result.

The full backend, installer, provisioning, desktop runtime and browser batteries were skipped: no backend, protocol,
installation or native lifecycle behavior changed, and the targeted checks cover the affected GUI paths. No new control
needs tooltip coverage. The docs edit changes ordinary prose only, without links, headings or executable examples, so
formatting was sufficient and no website build or preview was started.

Main advanced to `64ff9b8603e4af685eb58510dcc8bd16ac81fafa` during execution. A separate read-only audit examined every
upstream product change: terminal recovery and shutdown checkpointing preserve accepted-request ownership, retry
identities and the GUI's deadline. Their specification and TODO edits affect separate passages. No functional
interaction required rebasing this stack or rerunning its checks before delivery. Landing still needs to integrate both
sides with its then-current main.

## Review gate's outcome

Each PR received the required fresh-context adversarial review by gpt-6.1-sol at high effort. All findings were accepted
and fixed, including hidden-pill accessibility, timeout evidence through response-body reads, Replace's retry-identity
ownership and header width. The final PR1 review passed after the corrected artifact's deadline tests passed on both
engines; PR2's follow-up found no remaining actionable issue. Fresh own-model scope checks approved the deadline marker
and forced header layout adaptation. Commit and PR wording passed their independent cold reads. Nothing has been marked
ready or merged by this executor.

### Landing

Landed with one stylesheet fix at landing, second of the three plans landed together in this round. A launch, Restart or
Replace that takes more than two seconds now shows which host it is waiting on and for how long, and one that passes the
one-minute deadline says it is still waiting and may still finish, instead of "Not launched."

- #2970: the launcher's, quick launch's and template shortcuts' waiting status.
- #2974: the same for Restart and Replace, and the removed TODO entry.

#### What the landing fixed, and why

The UI's JavaScript tests failed on this plan: the quick-launch status pill's rounded corners were written as a literal
`999px`, and the stylesheet test refuses literals outside its token block. A token with that value already exists
(`--radius-pill`), so the landing used it and widened its comment to name both pills; the computed style is the same.
The executor's report does not list the JavaScript tests among its checks.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained faster-snapshot-encoder, colored-harness-marks, discord-link-in-app,
grid-drop-and-drag-notice, and PRs that only added TODO entries and plans. In this round, session-summary-beside-list
also edits the session row; a fresh-context review found the two in separate places (the waiting status sits inside the
row; the summary card is placed from the row's open button and the sidebar's edge), and nothing from
colored-harness-marks', grid-drop-and-drag-notice's or discord-link-in-app's row and stylesheet changes overlaps them.
The rebase conflicted only in implementation-notes sentences changed apart, resolved keeping every side's changes; a
check confirmed each commit changes exactly the sentences it changed when tested.

#### Things you should know

- The executor's report notes a host-terminal browser run that stopped before any test because the run's setup checked
  for the stack's first session while it was still being created. The landing saw the same race once in the previous
  round, independently; it is a problem in that run's setup, not in this plan.
- A trash dialog test (keyboard scrolling in the Delete all confirmation) failed repeatedly in this round's runs, more
  often with this plan in the stack than on main. It also failed once without this plan in the previous round, so it is
  not shown to be this plan's, but not cleared either; the checks below give the counts, and finding whether this plan's
  waiting status affects that scrolling is a follow-up.

#### Checks

This round landed vs16-emoji-width, launch-progress and session-summary-beside-list, stacked in that order on main
`5c5af756`. On that stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 403 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older reports), the docs website build (`bun run build` in `website/`), the workspace debug build and
  the UI's release web build: all passed. These ran before launch-progress's landing fix, a stylesheet value that
  computes to the same style; the builds were redone with it for the JavaScript run `c5047ec6`, the trash dialog runs
  `d30273b9` and `03eda759`, and launch-progress's own specs below.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `b14ee80d-08f8-4500-8471-e106fd12b13d`: all 842 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `b4981abf-b680-4336-8ed1-c1b993d4f947`: all 1031 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `83ad6a8c-ae24-43a4-a2a5-be982d905cb0`: 515 passed, 1 failed (the stylesheet test
  launch-progress's landing fix addresses); with the fix, run `c5047ec6-c334-493a-b9d8-4ad22ecd5b84`: all 516 passed.
- The vendored crates (vs16-emoji-width patches them): `cargo test` of vendored vte with `--features ansi`, run
  `c1feddd1-8351-46cd-a4dd-fe2a3c2c34d6` (62 passed), of vendored alacritty_terminal, run
  `ac66eb6f-33df-414f-8023-8e6f82d808ac` (165 passed), and of the paired tests, run
  `11056c7f-5f7e-41fb-8052-54cb6fdb7f15` (3 passed). The terminal emulator's ignored round-trip and hostile-snapshot
  tests in release (`cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E '...'`, the
  full-size round trips, the 300,000 short scripts and the 230,000 hostile blobs), which their documentation asks for
  when the decoder or the vendored patches change, run `c957bc92-29df-4bec-9178-06713631da5c`: all 10 passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux first on `PATH` (so the emulator's parity tests against tmux ran), no retries, on a machine with rootless Podman,
  run `68783337-311c-45d3-b891-a1dfc3eb9f29`: 5428 passed, 70 left out by the test runner (tests marked ignored), 3
  failed: two restart-gap session-listing tests and an OMP report test, which have failed the same way on main since
  round 169 found them (stop-checkpoint-burst's landing notes name them), in code none of this round's plans changes.
  They are recorded nowhere yet; the maintainer was told in conversation, and after three rounds finding their cause, or
  at least recording them, is a follow-up.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack before launch-progress's landing fix; Node 24.16 there, against 24.20 locally; the sandboxes' ssh to
  themselves worked, with a fix to their setup that lands separately), runs `268ae178-8cf1-49e9-bc0e-0185f774a4f7`,
  `fc0161c2-634f-412d-91b3-edae353e7a45`, `e57f9fe2-36de-43f7-aade-46eadb89f6ee`,
  `a865423e-2157-473d-bd78-5bafcce7811e`, `85787359-a26b-4ba5-886b-5db26f29c782`,
  `d09a94d4-9ba3-4ffe-852d-a1f9e540e9bc`, `0a181417-63c4-4824-a8cb-6346a34bc6c2`,
  `905592e5-268b-477d-807f-d8ea2dd8225f`, `f88dbc3b-0bd0-42bb-b708-9d04a42e31a1`,
  `0a8e20e1-eaac-43c9-a9d9-b7a27ee49d3b`: 2891 passed, 18 skipped by design, 3 failed. Then the browser suite's
  host-terminal run (every stack's terminals in terminal hosts, `FARHELM_E2E_SUITE_TERMINALS=host`) the same way, runs
  `7f6083bf-d51d-442c-88e1-5b70d51e3309`, `917531e9-bc85-48de-87c7-d92f0a5b42b4`,
  `89a737f6-3967-42aa-a519-a90238061970`, `76be9ed9-983d-442a-a852-38bfdfad7051`,
  `cda6b941-c943-4e8e-b96d-294bf94ae240`, `d0d688aa-0374-4cd4-b816-88b95b5b3234`,
  `0a773593-de91-4798-898a-bfd970f183ed`, `60cb802f-838e-4d99-95fd-b2642908dae4`,
  `57d262a1-fe43-4398-8ff0-53f5c9acbd4c`, `a4c9e736-ed83-48cc-8198-6b43fa19b22f`: 2050 passed, 75 skipped by the specs
  themselves on terminal hosts, 5 failed.
- Each failure re-ran three times on the same build, also on sandboxes (runs `de7f9716-ee9c-48b5-8577-9b97467bd32f`,
  `e6e3b26c-8418-4e4a-9cfc-c3c976c4c8bf`, and in the host-terminal run `df17440f-34ee-4932-89b3-6519ae4ef4b5`,
  `e049551b-554f-4faa-a60a-076f30d9358e`, `32c0bae4-b4da-457c-b221-e9111ffd593f`,
  `5988d115-8b2b-45b5-a1f1-6570ec14cc8c`, `816213fe-60b3-494f-b8f3-2316945399da`). Passed 3 of 3, each a single failure
  not recorded as a flake: "create-intent-key-rebinds-on-host-change" (WebKit, multi-host: a radio button did not take
  its value), "an old session's band hides until the next visit" (Chromium, a test route's response was already
  disposed), "an idle session already seen uses its row's status foreground" (WebKit; its status word came out white),
  and "header controls stay interactive and text stays selectable" (Chromium; double-clicking the version selected
  nothing). "Post-clone commands with carriage returns refuse edits but can be cleared" (WebKit) is recorded as a flake
  by an open draft PR (#2549) and failed once more in the three; that evidence goes to its record when it lands.
- "Trash header and global confirmation remain reachable" (Chromium; after PageDown from the Delete all confirmation's
  Cancel, its scroll area stays at the top) failed at both widths in the full suite and at width 360 in the
  host-terminal run, then once in nine re-runs (six over both widths, run `e6e3b26c`, one failure at width 1000; three
  at width 360 in the host-terminal run, `32c0bae4`, all passed). Twenty runs each on sandboxes (ten per width): main
  `5c5af756` without this round's plans passed all (run `7ce0fa35-3abc-400d-ab8d-5fc457bb0731`); main with
  vs16-emoji-width and launch-progress failed 2, both at width 1000 (run `d30273b9-fc29-4e19-b4c0-036465123b34`); the
  whole stack with launch-progress's landing fix failed 3, all at width 1000 (run
  `03eda759-efdb-41ff-956b-564f134ad83d`). Those counts point at launch-progress without settling it: in the previous
  round, on a stack without launch-progress whose code is now main, the same check failed once in a valid pass (width
  360; that round's notes called it a cleanup failure, which came after this check failed) and three more times in a
  pass whose ssh-reached second host never connected, which that round discarded. So the test can fail without
  launch-progress, but whether launch-progress makes it fail more often is not shown either way, and its cause is
  unknown. The landing judged it not a reason to hold the plan; the test is being recorded as a latent flake with these
  counts after this round, and checking whether launch-progress's waiting status (its once-a-second clock or its pill)
  affects the confirmation's scrolling is a follow-up.
- launch-progress's own specs after its landing fix, on both engines and in both the ordinary and host-terminal runs
  (runs `a7cfb2f2-53ee-462f-8aae-14a6de7f04d1`, `b28405ed-9d16-47c2-a2ce-ee1c8d1c0187`,
  `215b38cf-56ba-4c89-ad57-cce8bd5d3390`, `4d14e3a3-6f3e-477c-9ab6-f3cc5080787e`): all 56 passed.
- Reused: nothing. Not run: the doctests (no doc examples changed), the desktop smoke tests (no desktop-specific code
  changed) and the native Mac app (not runnable on Linux).

The executor's report names its commits before the landing's rebase and calls the PRs drafts; what landed is those
commits rebased onto current main, with the stylesheet fix in #2970's. Its checks do not include the JavaScript tests.
Nothing else in it became untrue at landing.
