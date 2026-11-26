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
