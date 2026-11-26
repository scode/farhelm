### What this was about

The maintainer approved eleven provisioning and runtime outcomes in four grouped PRs. All four are verified drafts
awaiting landing.

A newline in a resolved path could make Uninstall misread the layout and risk removing a host's state. It now refuses
that path and directs the user to Remove in Hosts. Update refused too late when a previously identified host answered
without its identity. It now refuses at planning or confirmation and says to wait for the identity to return, change the
host's destination, or remove the entry. Where the Mac installer cannot lock the Applications folder itself, it creates
an owner-only fallback lock so another local user cannot hold a newly created lock open and block installation.

Image inspection and leftover discovery could fail when Podman returned enough unused fields to hit Farhelm's output
limit. Farhelm now requests the identifiers and labels it uses, checks ownership of each leftover candidate, and still
requires a complete response across hosts. Invalid recipe titles are refused before provisioning or agent approval,
including control characters or titles over 64 KiB for a home-folder launch.

A heartbeat from the displaced desktop could interrupt its takeover notice. The notice now drains before the socket
closes if the socket can still deliver it. Cancellation while an attachment is being published now reports that the
result is unknown, since the file may still appear. A retry can leave two complete copies.

### Things to know

The three test changes fit the maintainer's roughly 20-line limit per gap. The new Delete assertion checks that the
pinned shell is classified as a tab root, and the stand-in's existing Delete assertion checks eventual exit. The
checkout tests check that a sandbox can use its home without setup and that Replace adds a repository suggestion without
aging the host's ordinary launch history. All eleven outcomes are recorded in the archive.

### Open questions and possible follow-ups

No maintainer decision is needed for landing. Verifying that Delete sends the actual hangup signal to a markerless tab
shell would need a signal-specific test through the Delete route. The new classification check guards the tab-root
selection.

### Pull requests

- Uninstall path refusal, Update identity guidance, and the private installer lock.
  [#5027](https://github.com/scode/farhelm/pull/5027/changes)
- Compact Podman replies and early recipe title validation. [#5031](https://github.com/scode/farhelm/pull/5031/changes)
- Desktop takeover notice and unknown attachment publication outcome.
  [#5034](https://github.com/scode/farhelm/pull/5034/changes)
- Regression checks for Delete tab roots, Replace history, and checkout setup.
  [#5038](https://github.com/scode/farhelm/pull/5038/changes)

### Checks

- Ran `cargo nextest run -p farhelm-helm -p farhelm-supervisor -p farhelm --lib --test e2e` with nine selected cases on
  rebased stack tip `c08eebe5`. All passed in recorder `2631527f-75d8-4a8f-b7d4-66dd71947102`. The selection covers the
  takeover race, both upload cancellation checks, Delete tab-root classification, the two recipe checkout cases
  (inherited default and home without setup), Replace's history, and the installed-host setup and admission-window
  controls. The recheck exercises shared supervisor code and fixtures changed by the hook handoff work on main.
- Reused four passing host-maintenance and installer cases from `a932f12d`, recorder
  `f12ac3f6-6320-4676-a8a5-d7c436389d7f`. The `cargo nextest run -p farhelm-helm -p farhelm-supervisor --lib` selection
  checks Uninstall path refusal, missing Update identity, Update against a skewed supervisor, and private creation of
  the fallback lock.
- Reused 40 passing cases from the Podman and early-title selection of `cargo nextest run -p farhelm-helm --lib` at
  `fb7814c0`, recorder `783cacbc-e7e5-4992-a27d-030ccb5858fe`. That run's REST title test stopped at missing billing
  confirmation and missed the title guard. The corrected exact REST case passed at `fc7f972e`, recorder
  `6f484913-58af-470e-bfe2-fd60a07a25e4`.
- Reused the unaffected UI and upload cases from the 59-pass
  `cargo nextest run -p farhelm-helm -p farhelm-supervisor --lib` selection at `f63b0ad4`, recorder
  `ed189559-33b1-4b63-9868-551c31a6e4ee`. Each draft's changed lines match its reviewed version after rebase. The
  selected cases above recheck the shared code touched on main.
- `cargo fmt --check` for the affected Rust crates, `git diff --check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, and `scripts/check-test-sleeps.py` with its isolated interpreter passed. The
  instruction checker reported one existing archive heading warning.
- Skipped the full battery because the selections exercise the changed paths and their shared dependencies. Podman
  coverage uses modeled responses, provisioning checks use isolated fixtures, and live macOS hangup was outside this
  run.

### Independent review

All four source reviews passed after corrections. Review led to changes in the Update error status and Uninstall
recovery wording and aligned the upload fixtures with the cancellation reply. The four reviewable changes match their
original diffs after rebase, with main's triage records preserved.

### Landing

#### What landed with it

This plan landed first of three in its round, before the spec and comment updates (`triage-spec-and-comment-updates`)
and the queue discards (`triage-queue-discards`), which change no code. Since its PRs were based, main gained the agent
request and container approval triage, the host menu and terminal triage, the hook warning and Antigravity fixes and
time limits for the screenshot comparison. The rebase met only conflicts in the shared triage records. The landing's
independent review found no interaction: in particular the plan's stronger Replace history test still tells its cases
apart after the agent request and container approval triage changed how a launch is recorded in repository history, and
this round's workspace run covers the plan's tests on the rebased tree, which the executor's runs did not.

#### Fixes made while landing

- **Two changelog drafts described the change in the wrong terms (the review's should-fix).** The Podman one named the
  mechanism ("reads less metadata"); it now says what stops failing for users (image retirement during a recipe's
  Rebuild or Delete, recipe builds, the leftover container list) and that the size that triggered it was not measured.
  The desktop one led with a window takeover race that cannot happen in the shipped app; it now leads with the upload
  cancel and tells the curator that its premise is unverified and the takeover part is not worth an entry. Both inside
  their own PRs, [#5031](https://github.com/scode/farhelm/pull/5031/changes) and
  [#5034](https://github.com/scode/farhelm/pull/5034/changes).

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the round's three plans stacked in queue order on main as it stood at the start of the landing (`491df5c5`), with the
shared triage records' conflicts kept both ways (by hand for one review queue index block in the spec plan's PR and the
open outcomes file in the discards plan's PR, then checked: the queue's 21 feedback files, its 21 index entries and the
21 open outcomes agree). Every other change is exactly what the executors pushed until the landing fixes, which change
only a SPEC.md sentence and two changelog drafts.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `0368f541-87a9-4191-8302-5b438567d5bf`): all 8765 passed, with no runtime `SKIPPED` lines. That includes the
  provisioning, Podman, upload, UI channel and runtime tests the plans added or changed, on the rebased tree.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `19013678-79f1-4d2e-9e1b-0271274e62f8`): of its 49 steps, 48 passed and one (paste) was skipped by design on Linux.
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format` (again after the fixes),
  `cargo fmt --all -- --check` and `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally:
  it fails only on over-long lines in the triage outcomes archive: main's earlier ones and the new entries of the
  provisioning, Podman and runtime plan (the other two plans' entries pass); no PR fixes them yet; a formatting-only one
  would, and I recommend it, as earlier rounds' notes did.

Reused across the fixes: every run above, since they change no code. The executors' runs were on older bases, and
everything they covered ran again above.

Skipped: the screenshot comparison and the desktop app's outer shell tests, since no drawing code or desktop shell code
changed; real Podman and Tensorlake hosts and a Mac run, which no automated check here reaches; and CI, which runs only
on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **Half of the desktop fix addresses a case users cannot hit.** The takeover notice fix in #5034 matters only when a
  second window takes over the first, which the shipped desktop app, with its one window, never does; the upload cancel
  half is the part users can meet, and its premise is unverified (the changelog draft now says both).
- **One regression in Uninstall would go unnoticed.** The new test checks Uninstall's new refusal of a path containing a
  newline on its own, so a later change that again stripped newlines before that check would pass the tests. The check
  is right today; I recommend no follow-up unless that part of Uninstall changes again.
- **Smaller points the review noticed.** SPEC_impl.md does not mention Uninstall's new refusal of a path with a newline;
  one Update message says an identity is "on record" when it was only seen at planning; the desktop channel's doc
  comment does not say its drain is now bounded by the write deadline; one archived triage note implies test coverage of
  a tab hangup path that does not exist; and the plan's new archive entries are not wrapped. None needs a follow-up on
  its own.
