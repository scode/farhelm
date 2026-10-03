## What this was about

Typing into any session on a host could freeze while that host was busy with session management. A host's supervisor
lets up to eight management operations run at once (stop, restart, delete, rename, opening or closing a tab, browsing
folders, searching repositories). With all eight in progress, for example several stops or deletes each giving a
session's programs a few seconds to exit, the next such request waited for a free slot inside the same loop that
delivers every keystroke, resize and detach for every session on that host. Typing froze until one of the eight
finished. Even a routine session-list refresh did this, and a freeze longer than the helm's 30-second list timeout made
the helm drop its connection to the host.

In the 2026-10-02 triage you decided four review findings about this, one each for the session list, stop, restart and
rename:

- the session list leaves the management limit entirely and gets a small limit of its own;
- stop, restart and rename are refused at once with a "try again" when every slot is taken, instead of waiting, with
  code comments saying why. The stop finding's fix also covers the requests that share the common admission path: tab
  open and close, folder browse and repository search.

Delete keeps its current behavior, since it already waits for its slot off the input path. This plan carried the four
findings out as four stacked draft PRs, one per finding.

## Things you should know

- **New message users will see.** On a busy host, stop, restart, rename, tab open and close, folder browse and
  repository search now fail at once with "this host is busy with other session operations; try again in a moment". It
  appears where those actions already show their errors (for example "stop: …" on the session row, and in the restart
  and restart-with controls). The refusal comes before the request changes anything, so clicking again is safe. Nothing
  retries automatically.
- **Large batches of deletes.** Deletes still wait for a slot, and a waiting delete gets each slot as it frees up. So
  while more than eight deletes are queued on one host, every other management request there is refused until the queue
  drains, not just at the peak. This follows from the triage decision (refuse the others, let delete wait) but was
  worked out when the plan was written, not discussed in triage; the code documents it rather than working around it.
  See the open questions.
- **Typing can still freeze for one other cause.** Creating a session, and the fresh-checkout check the helm runs before
  some creates, still wait on the same input path behind a running delete. Those findings were routed in the same triage
  to the planned TODO item "Keep session creation off the connection read loop", not to this plan.
- **The session list** now has its own limit of two lists being built at once. Management work cannot hold it up, and a
  list waiting for that limit does not hold up typing either. The limit is released before the reply is sent, so a helm
  connection that has stopped reading cannot tie it up for everyone else.
- **One helm change was needed.** Repository search in the helm deliberately never shows a supervisor's own error text,
  and used to replace every failure with "verify Git and the configured checkout root". A busy refusal now gets the busy
  sentence instead. The helm still only reads the kind of error the supervisor sent, never its text.
- **No protocol version change.** The refusal uses an error kind that already exists and already means "nothing
  happened, the same request will work later". Older helms already turn it into a 503 and show its text. The one
  difference with an older helm is that its repository search would still show the Git hint for a busy refusal.
- **Changelog.** Two entries describe the change for users: one for the session list and typing, one for the busy
  refusal. The restart and rename PRs extend the second entry's wording; each also adds a fragment marked as having no
  user-facing text of its own, which only points at that shared entry, because the project requires a fragment in every
  `fix:` PR.
- **One end-to-end test was renamed.** The test that fires two dozen renames at once and expected every one to succeed
  now accepts the busy refusal for some of them, and is named `concurrent_renames_past_the_admission_slots_all_answer`.
  Whether a rename waits on the input path is covered by a new supervisor unit test instead.

## Open questions and possible follow-ups

- **Is the delete-batch consequence acceptable?** As described above, a host draining a queue of more than eight deletes
  refuses every other management request until the queue is short again. The alternative is to have deletes refuse too,
  or give them their own limit, which would be a new decision. My recommendation is to accept it: batches that large are
  rare, and the refusal says what to do.
- **The review gate ran at the session's own effort, not an explicitly set high effort.** The plan's agreed gate was a
  fresh-context Opus 5.5 reviewer at high effort per PR. The agent mechanism used could set the model but not the
  effort, so each reviewer inherited this session's effort setting, which was not confirmed to be high. You can accept
  the reviews as they are, or ask for a follow-up re-review at explicitly high effort.
- **Should SPEC.md mention the refusal?** "Waiting between operations on one host" says management operations may wait
  for one another and lists what must never wait. It says nothing about a management request being refused as busy,
  which these PRs make a visible behavior. Nothing contradicts the spec, and the triage outcome was code only. A
  sentence there would stop a future reviewer from reporting the refusal as a regression from "may wait". My
  recommendation is to add one sentence; it is a separate docs change if you want it.
- **No automatic retry.** A refused action needs a second click. If busy hosts turn out to be common, the UI could retry
  once after a short pause. That is not part of what was decided.

## The PRs

1. [#1483](https://github.com/scode/farhelm/pull/1483/changes) — the session list gets its own small limit and no longer
   freezes typing on a busy host.
2. [#1486](https://github.com/scode/farhelm/pull/1486/changes) — stop, tab open and close, folder browse and repository
   search are refused as busy instead of waiting; adds the shared refusal step, the shared message, and the helm's
   repository-search mapping.
3. [#1490](https://github.com/scode/farhelm/pull/1490/changes) — restart uses the same refusal.
4. [#1493](https://github.com/scode/farhelm/pull/1493/changes) — rename uses the same refusal, keeping its single slot
   through the title change and the reply.

All four are drafts, stacked in that order, rebased onto current main.

## Checks

Run now (runner: the repository's test-run recorder with the pinned nextest and tmux, four slots, no retries):

- `cargo nextest run -p farhelm-supervisor --lib` (whole supervisor library) on PR 2 before its review fixes: run
  `b3f7097b`, 1028 passed.
- `cargo nextest run -p farhelm-supervisor -p farhelm-helm --lib -E 'package(farhelm-supervisor) |
  (package(farhelm-helm) & test(/^sessions::tests::/))'`
  (the whole supervisor library plus the helm's session request handling) at the stack tip, after every code change from
  the PR 1 to PR 3 reviews and before the PR 4 review fixes and the rebase: run `e2dbe5cf`, 1151 passed.
- `cargo nextest run -p farhelm --test e2e -E 'test(/^session_rename::/)'` (end-to-end rename module) on PR 4: run
  `0ed9258e`, 18 passed.
- After the PR 4 review fixes, the renamed end-to-end test and the supervisor rename test: run `a5d70837`, 2 passed.
- After rebasing onto current main,
  `cargo nextest run -p farhelm-supervisor -p farhelm-helm --lib -E
  'test(/^service::handlers::tests::/) | test(/^service::connection::tests::/) | (package(farhelm-helm) &
  test(/^sessions::tests::/))'`
  (the supervisor's request handling and connection tests plus the helm's session request handling): run `97c57a27`, 208
  passed.
- Per-PR module runs during development: `90b70791` (PR 1, 83 passed), `b9e7d44c` (PR 2, 205 passed), `c0c87c16` (PR 3,
  85 passed).
- Deliberate failure checks, each run with that PR's fix reverted to show its new test catches the old behavior; each
  failed as intended: `ec048829` and `b39ea0e5` (PR 1), `c34c11b9` (PR 2), `aeaed47f` (PR 3), `35e53bd5` (PR 4). These
  are expected failures, not flakes.
- `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings` on the four changed packages
  (`farhelm-supervisor`, `farhelm-helm`, `farhelm-proto`, `farhelm`); `cargo clippy -p farhelm --bins -- -D warnings`
  for the shipped binary's configuration; the test-sleep check (no unannotated delays);
  `python3 releasing/check-changelog.py format`; `dprint check` on the changed Markdown.

Reused: the whole-library and end-to-end rename runs above cover code that changed afterwards only in comments, test
documentation and a test name (PR 4 review fixes), plus a rebase onto main. What the rebase brought in was the terminal
clipboard limit, the terminal data-frame size limit, Claude's background-wait status, and planning documents. The
data-frame change touches the same supervisor connection file and protocol crate, but only a chunk-size constant and its
test, far from request admission; none of them interacts with this work, and the rebase was textually clean. The rebased
tip was re-checked with the targeted run `97c57a27`.

Skipped, with the reason:

- Browser end-to-end tests: no UI code changed, and the UI shows these errors through its existing error paths.
- Desktop, installer, provisioning and JavaScript checks: nothing in those areas changed.
- The full workspace nextest run: the change is confined to the supervisor's request admission and one helm route. A
  search found no other end-to-end test that sends more than eight management requests at once, which is the only
  situation where behavior differs.
- Doctests: no doc examples changed.
- Workspace-wide clippy: only the four packages above changed, and each was linted with all targets.

## Review gate

Each PR got a fresh-context Opus 5.5 reviewer asked to check general correctness, design fit, and idiomatic code against
that PR's triage decision and plan item, and to apply the repository's test-writing rules to changed tests. The
reviewers ran at the session's own effort setting (see the open questions). Every finding was fixed by restructuring
that PR, not by stacking corrections on top:

- PR 1: three findings. The list's limit is now released before the reply is sent, a test now covers a list waiting for
  its own limit, and two overstated comments were corrected.
- PR 2: five findings. These covered the helm's repository-search message, comments that described later PRs' changes
  too early, the wording of how long the old freeze lasted, a stale helm comment, and a simpler helper signature. The
  spec question above came from this review.
- PR 3: two findings, both comment-only: a misplaced doc comment and line wrapping.
- PR 4: five findings, all about documentation. They covered the renamed end-to-end test's claims, the current reason a
  rename keeps one slot (a busy refusal is only honest before the title changes), and two comments still describing the
  old wait.

No finding was declined.
