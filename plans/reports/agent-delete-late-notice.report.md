# Report: always post agent-Delete leftover-checkout notices to the notices bell

## What this was about

When a session is deleted and its managed checkout cannot be archived safely, Delete still removes the session and
leaves the folder on disk. SPEC.md requires that this never be silent: the person gets a notice naming the path and the
reason. For a Delete started from the app, the notice appears with the result. If that Delete outlived its reply
deadline, the notice goes to the notices bell instead.

A Delete that an agent asks for (`farhelm agent delete`) only sent the notice back to the asking agent. The agent might
not pass it on. And if the agent's request outlived its supervisor's reply deadline, nobody saw it at all, because the
helm only knows that a reply arrived, not whether anyone was still listening. The TODO entry asked to keep these
notices.

The maintainer decided during planning to keep it simple: always post an agent Delete's checkout notice to the notices
bell, with no change to the protocol and no attempt to tell whether the reply was still awaited.

The PR does that. After an agent Delete finishes, any checkout notice it produced is recorded in the notices bell, the
same way a late app Delete's notice is, as well as returned to the agent. This covers Deletes of an ordinary session and
of a session inside a container or sandbox environment. SPEC.md and SPEC_impl.md now describe it, the TODO entry is
removed, and a changelog fragment is added.

## Things you should know

- **The bell entry appears every time.** For an agent Delete that leaves a checkout behind, the bell entry appears even
  when the agent got the notice in time, so the same notice can reach the user twice: once through the agent and once
  through the bell. That is the decided behavior. The bell entry reads "Session deleted on ⟨host⟩" with the path and
  reason, and opening it marks it resolved.
- **Recording failures are logged, not reported.** If recording the bell entry fails (a database error, say), the helm
  logs it and the Delete still counts as done. The agent still gets its notice if it is still waiting for the reply. If
  its request already outlived the deadline, the notice then survives only in the helm's log. That is the same as when
  the bell write of a late app Delete fails, which also only logs it.
- **Nothing changed in the app or on the wire.** The app's notices bell already knew how to show this kind of notice.

## Open questions and possible follow-ups

None.

## PRs

- #5088 — fix: put an agent Delete's checkout notice in the notices bell.

## Checks

Run now, on the PR's commit:

- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `cargo check -p farhelm-helm --tests` and
  `cargo fmt -- --check` for the helm and protocol crates: clean.
- `dprint check` on SPEC.md, SPEC_impl.md and TODO.md, and `python3 releasing/check-changelog.py format`: passed.

Reused:

- `cargo nextest run -p farhelm-helm --lib` on the agent request and notices tests: 165 passed (recorded run
  dedcc472-203c-4244-b879-27c1e3fbeb41). It ran on the commit before the review's fixes, which changed only comments and
  spec text, so its coverage still applies.
- That selection includes the two extended tests:
  - an agent Delete of an ordinary session that leaves a checkout puts the notice in the bell and returns it to the
    agent;
  - the same holds for an agent Delete of a session inside an environment.

Skipped:

- The workspace and end-to-end batteries: the change is a few lines in the helm's agent Delete handling, which the tests
  above drive through the real request handler with scripted supervisors.

## Review gate

A single fresh-context Opus 5.5 reviewer at high effort, using the plan's general charter and the test-authoring
checklist, reviewed the PR in two rounds.

- **Round 1:** no correctness problems. It found one medium and five low documentation problems, all fixed:
  - **Medium:** SPEC_impl.md still said a timely reply adds nothing to the notices bell. It now singles out agent
    Deletes.
  - **Low:** SPEC.md's agent Delete paragraph and notices paragraph, two code docstrings and two test docstrings still
    described the old behavior.
- **Round 2:** found nothing.

### Landing

#### What landed with it

This round landed supervisor-tab-close-retry first (a failed terminal tab close keeps the tab and retries), then this
plan. The two touch separate code. Since this plan's base, main had gained a change keeping Replace with in the source
session's folder (#5081), a spec change letting Enter pick a list highlight that has scrolled out of view (#5082), Stop
and Restart ending an agent that outlived its killed terminal host (#5083), an unrelated container-host change (#5084)
and TODO entries (#5085, #5089). None touches the helm's handling of agent requests or the notices bell, so nothing
interacted. An independent reviewer and I confirmed the change does what the report says: one place covers Deletes of
ordinary sessions and of sessions in a container or sandbox environment, the bell gets the notice at most once per
deleted session, a failure to record it is logged, and nothing changed in the app or between the helm and the app.

#### Fixes made while landing

None. The independent review found nothing to fix.

#### Worth knowing

- **This corrects the report: the bell gets every notice an agent's Delete produces, not only checkout notices.** For a
  session in a container, that can be a "Session deleted on ⟨host⟩" entry saying that stopping its container could not
  be confirmed, with no checkout path in it. That is arguably more useful, but SPEC.md says only checkout notices go to
  the bell. If you want the spec to match, changing "checkout notice" to "Delete notice" in its agent Delete sentence
  would do it.
- **One case still loses the notice.** If the asking agent's request is cut off while its Delete is still running, the
  notice reaches neither the agent nor the bell. I did not check whether the Delete itself still finishes in that case.
  That is outside what the plan decided to cover. Not worth a follow-up unless it is seen to happen.

Nothing from the landing needs a follow-up unless you want the spec wording above changed; say so and it becomes a Plan
follow-ups entry.

#### Checks

Run once for the whole round, on supervisor-tab-close-retry and agent-delete-late-notice stacked in that order on main
as it stood when the landing started; what main gained since each plan's base is in each plan's notes. The only landing
fixes, two spec corrections to supervisor-tab-close-retry, change only SPEC.md and SPEC_impl.md, so these runs still
cover what merged. The test worker is an aarch64 Linux machine with a systemd user manager:

- On the test worker, the four clippy configurations (`cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets` with and without
  `--features agent`, each with `-D warnings`): clean.
- On the test worker, `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm-gui`,
  recorded run d8896d2e-d92c-4d1e-8121-ea0329db3284: 7,416 passed, 17 skipped (tests marked ignored). That is every test
  of the shared message types, the helm, the supervisor and the desktop app, wider than either executor's selection.
- On the test worker, the end-to-end tests of host tabs (including supervisor-tab-close-retry's three new ones),
  terminal tabs, tab edge cases, host terminals and every test whose name contains `delete`
  (`cargo nextest run -p farhelm --test e2e` filtered to `host_tabs`, `terminal_tabs`, `tab_lifecycle_edges`,
  `host_terminals` and `delete`), recorded run 0ffb2472-3094-47e8-9b62-3e09fc4aa544: 85 passed, and none skipped for
  lack of systemd.
- Locally, `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown: passed, and `dprint check` on both specs again after
  supervisor-tab-close-retry's spec fixes.

Reused: nothing. Skipped: the screenshot comparison and the desktop app's end-to-end smoke test, since nothing drawn
changed; the Podman suites, since neither plan changes how containers are started or stopped. agent-delete-late-notice's
Delete of a session in a container or sandbox is covered by the helm tests above, which use a stand-in for the
environment instead of a real container.
