## What this was about

The browser test that double-clicks the version text in the app's header and checks that part of it got selected
(`header controls stay interactive and text stays selectable beside the forced-class spacer`, in the window-chrome spec)
failed now and then in Chromium: once in a whole browser run and once in 20 repeats on a Tensorlake sandbox, while
landing nonblocking-session-actions on 2026-11-10. That plan freed the page while sessions launch, and it was suspected
because it added a page-wide listener for key and pointer presses and was thought to have changed header styles. It is
not the cause, and the test is now fixed.

On load the page selects the newest session, and the helm and supervisors that every browser test run starts always have
one. When that session's terminal appears, it moves keyboard focus into the terminal's hidden input as long as focus is
still on the page body, and double-clicking plain text leaves focus there. A terminal that appears just after the
double-click therefore takes focus, and Chromium replaces the text selection with the input's empty one. As you decided
when planning, only the test changed: it now waits until that terminal has appeared and taken focus before it
double-clicks.

How it was established, following the procedure you agreed:

- Locally, 100 repeats of the test on Chromium, on current main and on the revision just before
  nonblocking-session-actions (`3ca47a1d`, built in its own workspace): all 200 passed.
- Since neither failed locally, on Tensorlake sandboxes at your request (one sandbox per run, at most two at once): 12
  of 100 failed on `3ca47a1d` and 19 of 100 on main, all with the same assertion. So the failure predates
  nonblocking-session-actions, and neither its press listener nor anything else in it is the cause.
- On Tensorlake, a temporary copy of the test that recorded focus, selection and pointer events with timestamps (never
  committed) failed 6 of 50 times. In every failure the double-click had selected part of the version text, and then the
  terminal's input took focus 12 to 28 ms later and the selection went empty. In passing runs the same focus move
  happened before the first click.
- The fixed test passed 100 of 100 on Tensorlake, where main had failed 19 of 100.

The original entry also said that nonblocking-session-actions changed header CSS. It did not: its style changes touched
only the session list's Stop progress and the launcher, nothing in the header.

## Things you should know

- The page behavior behind the flake is unchanged, as you decided: if a session's terminal appears just after you select
  text in the header (or anywhere else on the page body), the selection is cleared. In practice this needs a selection
  made in the moment between the page loading and the terminal appearing.
- The test now relies on the auto-selected session's terminal taking focus. If the session a test run starts with ever
  cannot take focus (its program exited, or its host is unreachable), this test times out with a message saying focus
  never arrived, rather than failing on the selection. The reviewer flagged this as a tradeoff the decision implies; if
  it ever shows up, the test can open a session of its own instead, as its sibling test does.
- The plan said an instrumented copy of the test could not run on Tensorlake. It can: the run packs the tracked files'
  contents as they are on disk, so an uncommitted edit to a test is included. That is how the diagnosis above was
  gathered on the machines where the failure shows.
- The failure never showed on the shared development machine (0 of 200) but did on the sandboxes (12 to 19 of 100). Why
  the sandboxes more often reach the double-click before the terminal appears was not investigated.
- The Tensorlake template was built fresh for this plan and deleted at the end, together with every sandbox and snapshot
  the runs made.

## Open questions and possible follow-ups

- Whether the page should keep a selection the user just made when a terminal appears, for example by not moving focus
  into the terminal while the page has a non-empty text selection. Not done here, per your decision; a possible
  follow-up if it matters outside tests.
- `.agents/tensorlake.md` says a run "packs tracked files only" and asks for an empty working copy, which is why the
  plan expected an instrumented test copy not to reach the sandbox. A sentence there saying that uncommitted edits to
  tracked files are packed would save the next investigation the detour.

## The PRs

- [#3350](https://github.com/scode/farhelm/pull/3350/changes): wait for terminal focus before double-clicking the header
  version, and remove this investigation's TODO entry.

## Checks run, reused and skipped

All test runs went through the test-run recorder, in the mode it uses for arbitrary commands (its browser mode always
runs both engines and cannot select one). Single-test repeats used
`npx playwright test --project=chromium-window-chrome -g "text stays selectable" --repeat-each=<n> --retries=0`; the
Tensorlake runs used the same selection through
`scripts/tensorlake/sandbox-tests.ts run --grep "text stays selectable" --repeat <n> chromium-window-chrome`, whose
shards record the same way. Every sandbox shard reported a running systemd user manager, lingering enabled and working
self-ssh. The sandboxes' Node version differs slightly from the development machine's, as `.agents/tensorlake.md` notes.

- Local, main, 100 repeats: 100 passed (`02b1cda3-67cb-44ad-9378-70b55e1af68e`).
- Local, `3ca47a1d`, 100 repeats: 100 passed (`8fe79994-128e-4b91-b97d-4c2e98d56ef7`).
- Tensorlake template check (a quick run of a couple of specs on the freshly built sandbox image): 20 passed
  (`551c55ba-1740-43f2-88b5-bc9a0520f435`).
- Tensorlake, `3ca47a1d`, 100 repeats: 12 failed, 88 passed (`77b014b7-e330-4562-a402-f222d5e2cbd2`).
- Tensorlake, main, 100 repeats: 19 failed, 81 passed (`a1ce0450-7e80-4e88-8f34-29fa1e6ac556`).
- Tensorlake, main with the instrumented copy, 50 repeats: 6 failed, 44 passed (`8f97193d-753e-4f38-9ff2-8ca956916225`).
- Tensorlake, main with the fix, 100 repeats: 100 passed (`6f8e8490-dd02-4517-9663-f299c8037946`).
- Local, the whole window-chrome spec on Chromium and WebKit with the fix: 12 passed
  (`1ec1e320-0de0-4660-b0e1-251e3c0e69ab`). After the review's two small changes (an explicit error when the selected
  row names no session, in place of passing an empty id on, and rewrapped comments), the fixed test passed 3 local
  repeats (`478dba0f-4fa9-459f-9313-56d283dfaf1f`); the Tensorlake and whole-file runs above are reused for them, since
  the waits and assertions are the same.
- The failing runs' summaries, consoles and returned run records are retained privately; they carry the assertion and
  the instrumented event logs, but no browser traces came back from the sandboxes.
- The isolated test-sleep checker (424 delays, none without a rationale) and `dprint check` on `TODO.md` passed.
- Skipped: Rust tests and builds beyond what the browser runs needed (no Rust change), a changelog fragment (test-only
  change), and WebKit repeats (the failure was reported and reproduced on Chromium; WebKit ran the whole spec file).

## Review gate's outcome

The required single fresh-context Opus 5.5 reviewer at high effort, given the full test-authoring checklist, found no
correctness or design defects. It confirmed that the selector matches the session row, that the auto-selection cannot
change to another session during the test, that the readiness wait includes the focus handoff the race is about, and
that no later focus move applies to this test. Of its four minor findings, two small ones were applied (the explicit
error and the comment wrapping), the dependency on the startup session taking focus was accepted as described above, and
its reminder to verify on Tensorlake before delivering is met by the 100-of-100 run. The commit message went through two
cold reads.
