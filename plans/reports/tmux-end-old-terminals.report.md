## What this was about

You asked for tmux and xterm.js to be removed from Farhelm completely, in four chained plans. This was the first: the
running product. Since v0.34.0 every new terminal runs in a terminal host and is drawn by Farhelm's own grid, but old
sessions could still run in each supervisor's private tmux server, drawn by the vendored xterm.js, and earlier releases
warned that a coming release would end them. This stack is that release's change, plus the removal of everything in the
supervisor, the helm, the protocol and the page that existed only because two kinds of terminal coexisted.

It blocked once halfway, on the size limit the plan carried. The landing monitor raised the limit for you, and you then
dropped line-count limits from these plans altogether, since a planning agent had added them without your intent. The
stack has six PRs: the desktop clipboard fix was small enough to fold into the fifth, as the plan allowed, and the sixth
records a flaky test that predates the stack, found while checking it.

What lands, in product terms:

- A starting supervisor ends the old private tmux server without running any tmux program: it connects to the server's
  socket, checks through the socket's peer credentials that the process answering is this user's tmux server, and ends
  it. Every session whose agent was still running in an old terminal becomes interrupted, its agent and the background
  jobs Farhelm can attribute to it are stopped, and it gets one notification of a new kind saying its old terminal was
  ended by this release and that Restart starts it fresh in a new terminal. That notice stays until read or cleared; the
  earlier "move to new terminals" warnings are resolved. A failure here never stops the supervisor from starting (your
  "start anyway" decision). The step has a 15-second budget, under the 30 seconds the helm waits for a restarted
  supervisor during an update.
- No scrollback is carried over: an ended session restarts into an empty terminal. The interrupted card and the specs
  now say an interruption comes from a host reboot or a Farhelm update.
- The supervisor's tmux code is gone, apart from that startup step, the check of which tmux version is good enough
  (setup, the desktop app and remote provisioning still use tmux until the next plan), and keeping Farhelm's own tmux
  variables out of a session for users who run their own tmux. `farhelm supervisor run --tmux` is removed.
- The protocol no longer names a terminal kind or a stream kind, and the helm no longer relays raw byte streams. The
  protocol version goes from 76 to 77, so after the update every remote host shows "needs update" until it is updated
  (you accepted this when planning).
- The page draws every terminal with the grid; xterm.js, its add-ons and their license notices are gone (code ported
  from xterm.js into the grid stays, with attributions). The session view loses the "move to new terminals" band and the
  terminal kinds in its details bar. The docs page "Move sessions to new terminals" (same URL) now explains what this
  release did to old terminals, and the installer's update message no longer promises that sessions keep running.
- The desktop app now passes a program's clipboard copies up to 8 MiB, the grid's own bound, instead of dropping those
  over 256 KiB (a limit sized for xterm.js).
- The release brick test no longer reads a session's terminal kind from the session details; it keeps its existing check
  of the kind stored in the supervisor's database against the running terminal process.

The TODO.md entries the plan names are removed (the terminal-kind simplification entry and the two xterm.js entries),
and about 40 review-queue items whose code is gone were removed with it (of the two the plan called uncertain, the
pane-root cleanup gap was removed as moot, and the oversized-grid attach item was narrowed to its supervisor half, which
survives).

## Things you should know

- **Two breaking changelog entries.** One says old terminals are ended on upgrade, and (since #3368) that a service unit
  written by hand must drop `--tmux`, or the supervisor will not start. The other, for the protocol change, says every
  remote host needs updating and that a browser tab left open across the update must be reloaded. They are separate
  because they ask for different actions.
- **A browser tab open across the update must be reloaded.** The review found that a page loaded before this update,
  talking to the updated helm, would draw its terminal as garbage if it reconnected. The page already stops reconnecting
  on its own a few seconds after it sees that its build differs from the helm's, and then says "press Reconnect now, or
  reload the page"; a reconnect in those seconds, or a manual one, misdraws. The stack documents that such a page must
  be reloaded and says so in the changelog entry. I did not keep a compatibility path for one release, because a later
  change would have to delete it.
- **Sessions whose old agent had already exited** get no notice: they keep their stored outcome, but their last screen,
  which lived in the tmux server, is gone. Restart starts them in a new terminal. Old shell tabs end with the server
  too; a session that lost only shell tabs gets no notice (your decision), and those tabs simply disappear.
- **Accepted residuals in the startup step** (from its review): when the old server was already gone, or Farhelm cannot
  tell which old shell tab a background job belonged to, that job is stopped only when its session is deleted (Delete
  stops everything of a session); a crash between interrupting a session and writing its notice loses the notice; and an
  old session whose creation had not finished preparing gets the notice and then a refused Restart, which leaves
  deleting it as the way out.
- **The shipped release announcement `0.37.0-old-terminal-support`** ("a future release will forcibly terminate them")
  is stale from this release on. It stays as shipped (see the open questions).
- **Test coverage that went with tmux.** Supervisor unit tests now run on real terminal hosts (the unit test binary
  doubles as the host program). Tests whose subject was tmux or byte streams were deleted with their code. Lost, not
  ported, and listed as follow-ups below: the check that memory stays flat while a viewer is stalled, two file-upload
  tests beside a busy or deleted session, and three tests where terminal hosts already behaved differently from the
  specification before this stack (creating a session or opening a tab in a folder that cannot be entered, attaching at
  size 0x0); SPEC_impl lists those three as known gaps.
- **Not verified on a Mac.** No macOS machine was used: the desktop app's larger clipboard limit was checked by the
  helm's unit tests, not in the app itself, and the release brick test's changed prompts were checked by its harness
  tests only. The brick test's first real run after this lands is the next release's.
- **Frozen material edited, as the plan allowed**: the README hero, demo video and docs-screenshot harnesses and the
  otherwise frozen release-shots test now run on new terminals (#3360); nothing was re-captured or published.
- **Left for the later plans**, by design: tmux-era wording in some end-to-end test comments and assertion messages,
  about 40 test fixtures that still insert old-kind rows (they are the input of the startup conversion), other docs
  pages that still mention old terminals (their links still resolve), and the install, tooling and docs work of plans 2
  to 4.
- **Size, for information.** By the plan's counting rule the five code PRs add about 1,140, 1,540, 1,740, 3,760 and
  2,800 lines, about 11,000 in all, against roughly 47,000 deleted. That is over the 8,000 the block's answer set: you
  told this plan's executor to ignore its limit, and dropped the limits from the three waiting tmux plans in #3366. The
  first three counts are lower than the ones in the blocked question (about 1,300, 1,650 and 1,750) because they are now
  measured with one script for all PRs, which treats lines moved unchanged more strictly. #3368 grew most: supervisor
  unit tests ported from tmux onto hosts, and a rename that collapsed the two-kind terminal type.

## Open questions and possible follow-ups

- **Retire the 0.37.0 release announcement?** Options: leave it (it is history in the release notes, but it still
  promises a future that has now happened), or retire it in a small follow-up. I recommend retiring it, since it now
  describes something that is already done.
- **Re-add the lost checks on terminal hosts?** The stalled-viewer memory check guards against a real failure (memory
  growing while a slow viewer cannot keep up); I recommend porting it to hosts as a follow-up. The two upload tests are
  worth porting with it. The three known gaps are host behavior that predates this stack, including that a host tab
  started in a folder that cannot be entered reports a failure naming `systemd-run` rather than the folder; I recommend
  one follow-up that makes hosts refuse those cases with a clear message and brings back the tests.
- **Eight grid-component tests fail on main as well**: the container file-path hover tests, on both engines, in the
  static grid-component suite. They fail the same way on the code before this stack (checked against the base commit),
  so they were left alone. I recommend fixing rather than retiring them, since they are the only static coverage of
  hovering a file path inside a container session.
- **A browser test flakes on main too, and is now recorded as a flake.**
  `header progress keeps controls inside a 650px pane with a long host` (Restart and Replace progress) failed with
  "Route is already handled" from the test's own network stub: about 1 in 20 on each engine with the stack, and 2 in 20
  on WebKit (0 in 20 on Chromium) on main without it. The repository already has a helper that makes such stubs safe,
  and this spec does not use it. Recorded in TODO.md's Deflake bucket, FLAKES.md and the sweep's known-flakes list
  (#3370), not fixed here. Two other tests failed once each at the tip and passed every rerun (Restart With withdrawing
  consent, 20 of 20 per engine; a WebKit provisioning menu test, 10 of 10); neither spec is touched by the stack, and
  one observation is not recorded as a flake.
- **A small simplification not done**: the helm still starts pausing a terminal stream it cannot deliver fast enough
  only once the supervisor has confirmed the attachment, an ordering that used to matter for byte streams and no longer
  does. Starting it earlier would remove a little code but change when a pause can be sent. I recommend leaving it: it
  brings users nothing.

## The PRs

- [#3358](https://github.com/scode/farhelm/pull/3358/changes): run the supervisor's tests on terminal hosts instead of
  tmux.
- [#3360](https://github.com/scode/farhelm/pull/3360/changes): stop the supervisor creating tmux terminals.
- [#3362](https://github.com/scode/farhelm/pull/3362/changes): end old terminals at startup (breaking).
- [#3368](https://github.com/scode/farhelm/pull/3368/changes): remove the supervisor's tmux driver and `--tmux`
  (breaking).
- [#3369](https://github.com/scode/farhelm/pull/3369/changes): remove xterm.js and the terminal kinds from the page and
  protocol, with the protocol bump and the desktop clipboard fix (breaking).
- [#3370](https://github.com/scode/farhelm/pull/3370/changes): record the long-host header progress test as a flake
  (TODO.md, FLAKES.md, the deflake sweep's exclusion list).

## Checks run, reused and skipped

Slow end-to-end runs were on Tensorlake sandboxes at your request, at most 10 at once; the sandboxes' Node is 24.16
against 24.20 on the development machine, which these tests do not depend on. Sandbox runs went through the test
recorder like local ones, as generic runs. Every shard reported a running systemd user manager and working self-ssh; the
Rust shards also had Podman 4.9.3 with the production base image.

At the tip of the stack (#3369, after a rebase onto a main that had gained the work letting agents in sandboxes and
containers manage their sessions):

- Whole browser suite, both engines, 7 shards: 3,310 passed, 14 skipped, 2 failed (`1dd5986b`, `d6ff5485`, `4ac237a6`,
  `63ae0313`, `06b340d8`, `a54b465d`, `4d1e3a45`). The two failures (Restart With withdrawing consent and the WebKit
  provisioning menu test, under open questions) passed 10 of 10 each on a narrow rerun of the same build (`dd963aff`,
  `95414e08`); the 20-repeat runs of the Restart spec came later. Their failed runs are kept.
- Whole Rust workspace from a prebuilt test bundle, 2 shards: 5,802 of 5,803 passed (`dc7241c5`, `748531a1`), including
  the real Podman environment test. The one failure, a compile-time check of a test macro, needs `cargo`, which the
  sandboxes do not have; it passed locally (`73dcf689`). The test that a user's own tmux inside a session keeps its
  jobs, skipped there for want of a system tmux, passed locally (`890c9f32`). One check of the OMP agent harness skipped
  there because the sandboxes lack the JavaScript runtime it needs, and it was not run elsewhere (see Skipped).
- After #3369's review fixes, a fresh build: terminal, attachments, clipboard and grid mount specs on both engines, 211
  passed, 9 skipped (clipboard skips WebKit by design) (`e5d905c0`, `05dfd381`, `4ba1f26c`, `681326c4`).
- On the final tree, the Restart, Restart and Replace progress, and grid mount specs on both engines: 1 of 50 test runs
  failed (the long-host flake above; `c8d30150`, `9d4d0aa6`, `85fb984a`, `6cfdb804`), then 20 repeats of both Restart
  specs on each engine (`73835b3f`, `e5e41d70`, `5ae50998`, `4611265d`) and 20 of the flaky test on main (`e3b624fc`,
  `0fee0628`).
- `cargo clippy --all-targets -- -D warnings` on the rebased stack.

Per PR, run locally through the recorder:

- #3358: supervisor unit tests on hosts, 1,791 passed (`1df0e867`); the moved end-to-end tests and the four real-agent
  audits (Claude and Codex, two each) passed (`b5597f1c`, `bb264a1c`, `1df4f83d`, `6db3642d`).
- #3360: supervisor unit tests 1,781 passed (`8cb7fc91`); the farhelm package 809 passed (`77c4f423`); 14 browser specs
  499 passed (`fbb35e94`); the browser stack cleanup test (`60d34e6c`). One end-to-end failure under shared load (a
  fresh GitHub token check) passed 10 of 10 alone (`2dae36f0` kept, `d0cf75e0`).
- #3362: supervisor unit tests 1,778 passed (`7a098a79`); the old-terminal end-to-end tests (`123dab0e`, `ce8323cd`);
  installer test (`5d7a087e`); UI and wording pins (`43d2ab24`, `972ae339`, `86acd463`); the session view, status and
  session operation tests after the first rebase, 54 passed (`c750cf35`).
- #3368: supervisor unit tests 1,595 passed after the rebase (`7fa3f932`; earlier `5c73bc09` had three test bugs from
  the change, fixed, and `a7b70ae5` one timeout under shared load, below; both kept); end-to-end binary 449 passed
  (`1079d05f`), agent requests and old terminals 63 passed (`54b82711`), 131 passed (`4864a9cf`); terminal emulator 118
  (`11d20cec`); farhelm binary 197 (`a0f24eed`). One supervisor test missed its 10-second reply once under shared load
  and passed 10 of 10 alone (`da247132`).
- #3369: proto, helm, UI and supervisor unit tests 4,605 passed after the rebase (`b274267c`, two failures from an
  out-of-date build, green on rerun in `39e6631b`), then proto, helm and UI 3,012 and UI 990 after the fix rounds
  (`9446a22e`, `bc99713f`); 14 touched end-to-end modules 250 passed (`593778d2`); JS harness 527 (`3df39d9e`);
  brick-test harness 260 (`361f905e`); grid-component suite 160 passed plus the 8 known failures (`e392d5d7`, and
  `6e7f8019` showing the 8 fail on the base commit too); an earlier full browser suite on Tensorlake before the rebase,
  3,295 passed, superseded by the tip run above.

Static checks on every PR as it changed: `cargo fmt --all -- --check`, both clippy runs,
`cargo check -p farhelm-ui --features desktop`, the desktop asset check, `dprint check`,
`python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and the isolated test-sleep checker;
`bash -n` and shellcheck on the changed scripts.

Reused: the per-PR runs of #3358 to #3362 for the rebases onto newer main. The first rebase only brought UI and docs
changes, re-checked with clippy and the session view tests; the second, onto the environment work, was re-checked by
compiling every PR and by #3368's and #3369's runs after it, plus the whole Rust and browser suites at the tip.

Skipped: the desktop smoke (only its `--tmux` argument went; real supervisor processes in the end-to-end suite run
without it); nothing for the docs website beyond its builds in #3362 and #3369 (all links valid); the old-terminal
browser set, deleted with the old terminals; macOS-only checks (no macOS machine in this run; see above); the OMP
harness check above. #3370 changes only TODO.md, FLAKES.md and the sweep's exclusion list; `dprint check` covers it.

## Review gate

Every PR had the required fresh-context review by Opus 5.5 at high effort, and its findings were fixed before pushing.
#3358 to #3362 were reviewed before the block (8, 6 and 9 findings, none high; all fixed, or accepted as the residuals
listed above). #3368's review found no behavior regression, only leftovers (dangling references, dead test-harness
branches, a weakened test, a deleted doc comment, code that only existed for two terminal kinds); all were fixed, and
because the fixes reshaped types, a second review of just the fixes found nothing wrong beyond small wording, also
fixed. #3369's review found a wrong protocol number left by the rebase, references to removed code, and the open-tab
problem above; all fixed, and a second review of the fixes found only wording, fixed. Not followed: moving the helm's
pause to start before the attach reply (see the open questions). #3370, the flake record, had the same review: no
blocking finding; its evidence paragraph was corrected to the recorded tree status, tmux identities and locale, and the
cause wording made neutral. Commit messages each went through cold reads (up to three).
