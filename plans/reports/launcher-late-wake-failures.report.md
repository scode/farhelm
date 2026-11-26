### What this was about

When you launch a session on a suspended Tensorlake sandbox or a stopped Podman container, the session launcher first
wakes the host and holds your draft while it waits. The helm answers that wake request within 20 seconds, saying it
"carries on" if the provider is still working. A wake that failed after those 20 seconds reached only the helm's log:
the launcher kept waiting for its full minute and then gave a stale or wrong reason ("did not connect"). The same
happened when the failure repeated an earlier error word for word, or when it failed before the host even showed it was
starting (a slow container check over SSH). You decided that the launcher should get the outcome of its own wake through
the hosts list the helm already pushes to the app window, and that a container's wake should be worded as starting, not
resuming.

What was built: the launcher now labels each wake request with a tag of its own. The helm records how that tagged wake
ended (finished or failed, with the reason) on the host's entry in the hosts list, and the launcher, seeing a failure
under its own tag, brings your draft back at once with the real reason. Everything else stays as it was: the 20-second
answer; the launcher's one-minute limit still covers waiting on a host that another operation holds and waiting for a
new connection; a wake that someone else finishes still lets the launch go ahead; and wakes started from the Hosts panel
or a session's own controls send no tag, so their late failures still reach only the log. When starting a stopped
container fails, the launcher now says starting it failed rather than resuming it; the sandbox wording is unchanged.

### Things you should know

The plan did not settle the points below; they were decided during execution, and each is one you may want to revisit.

- **Only the latest tagged outcome per host is kept, in the helm's memory.** The next tagged wake of that host replaces
  it, and restarting the desktop app (which runs the helm) forgets it. That is enough for the launcher, which only ever
  asks about the wake it is waiting on.
- **A busy refusal is never recorded as a failure.** When another operation holds the host, the launcher already retries
  every second within its minute; recording that refusal would make it give up on a host that only needed a wait.
- **If the code that carries out a wake crashes** (a bug, not a provider failure), nothing is recorded and the launcher
  waits out its minute as before.
- **Once the host has connected**, a failure recorded under the launcher's tag no longer cancels the launch, so a late
  report can never kill a launch that is already going ahead.
- **A malformed wake request is now refused.** A request that claims to carry a body but sends invalid JSON gets a "bad
  request" answer instead of being treated as an untagged wake. Requests with no body, or an empty one, behave as
  before; nothing in Farhelm sends a malformed one.
- **Only the launcher's wakes record an outcome.** Stopping or suspending a host, and the start the helm makes on its
  own when it finds a container stopped outside Farhelm, report failures as before (in the host's state and the log).
- **The release note.** The plan called for no release note. The second PR's changelog fragment drafts one anyway (the
  launcher now fails at once with the real reason), because users will notice it; I recommend the curator keep it. The
  first PR's fragment says it has no visible effect on its own.

### Open questions and possible follow-ups

- The Hosts panel's own Resume and Start still leave a late failure only in the log. They could send a tag too and show
  the reason on the host's row, if that is wanted.

### The PRs

- #5079 `feat: report a tagged resume's outcome in the hosts list`: the helm accepts an optional tag on a wake request
  and lists how the tagged wake ended on the host's entry.
- #5092 `fix: report a launch's late wake failure at once`: the launcher tags its wake, acts on its own outcome, words a
  container's start correctly, adds tests and a screenshot in both appearances, updates SPEC.md, and removes the TODO.md
  entry.

### Checks

Run now, on the stack rebased onto the latest main:
`cargo clippy -p farhelm-proto -p farhelm-helm -p farhelm-gui -p farhelm-desktop --all-targets -- -D warnings` (clean),
and through the recorder (run e299c2ef-e506-4abe-9366-8ceb69fe80d4, 715 passed):

```
cargo nextest run -p farhelm-helm -p farhelm-gui -p farhelm-desktop --lib -E 'test(/tagged_resume|lifecycle|host_list_serializes|http_contract|the_helms_pinned_listing_decodes/) | test(/launcher::/) | test(/hosts::environment_tests::/) | test(/environment_ui/) | test(/goldens::/)'
```

That covers the helm's wake tests (a late tagged failure recorded, a success recorded, an immediate refusal recorded, a
busy refusal and untagged wakes not recorded, a later wake replacing an earlier outcome, the hosts list reaching the
app), the recorded example of the hosts list that the helm and the app are both tested against, and the launcher's wake
tests (one tag per wake, a late failure before Starting for a container, repeated identical failures for a sandbox and a
container, a direct start refusal, other tags and a ready outcome ignored). Also run: `dprint check` on SPEC.md,
SPEC_impl.md and TODO.md, `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check`, and the
test-sleep check (no unannotated delays).

Reused, from before the rebase: the screenshot of the new scene (a stopped container's late start failure), rendered in
light and dark, inspected and published, and a full screenshot update that showed no other picture changed. Recorder
runs 1f638c91-45c1-4113-9c72-68998cd52e7f (the helm's wake tests, 26 passed), 2c4aaaeb-4735-4542-abcd-8b59b39ced7b (the
wake request and outcome formats, 13 passed), 712fa8e1-72a7-42b5-98f5-89eacecca46f and
ce4bd5cd-0b7a-4326-9c59-9965a9e6dc11 (every desktop and app test that reads the recorded example of the hosts list, 188
and 244 passed), and c235a3e8-d195-43fa-9a9f-55c70640d5b8 (the launcher, host environment and provider wording tests,
652 passed). The rebase brought in no change to the wake route, the hosts list's wake field, the example hosts list or
the launcher's wake code (the changes that landed touch other launcher screens, Settings and custom agents), so these
still apply; the run above repeats the parts most likely to be affected.

Skipped: re-rendering the screenshots after the rebase. The changes that landed since touch other launcher scenes; the
rebased run checks that every scene still builds, but not its rendered pictures. Also skipped: the workspace-wide Rust
run and the end-to-end suites, since the change stays in the helm's wake route, the hosts list and the launcher, all
covered above; the real-Podman end-to-end test, which needs a Podman host and sends an untagged wake whose behavior is
unchanged.

### Review gate

Each PR was reviewed by a fresh-context Opus 5.5 agent at high effort. #5079: nothing blocking; it asked for the docs
and SPEC_impl.md to say that a crashed wake records nothing and why the last record wins, and for the other readers of
the shared hosts fixture to be run (done, 244 passed). #5092: nothing blocking; it asked for a corrected changelog
sentence, two SPEC_impl.md clarifications (a failure after the host connected changes nothing; the time on a tag comes
from the app, not the helm) and one doc fix, all done.
