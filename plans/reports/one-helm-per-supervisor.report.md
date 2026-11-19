## What this was about

A second Farhelm could add a host that another Farhelm already managed and start controlling its sessions. The agreed
rule gives each supervisor one managing Farhelm. The supervisor remembers that owner across restarts, updates and
reconnects. A host without an owner is claimed by the first Farhelm that connects to manage it, including after an
upgrade.

## Things you should know

Add host asks before taking over a host, even when the confirmation for setting up a new host is disabled. A host
already listed shows the other Farhelm's tag and when it connected, with Take over in its actions menu. If Farhelm loses
its registry, choose Take over on the local host row to regain control of its existing sessions. Take over ends the
previous Farhelm's connection and leaves the host's sessions running. Host Setup, Update and uninstall are refused while
another Farhelm owns the host. The installed desktop app still offers its own release check on the local row.

A takeover approval covers the supervisor and owner the app showed. If a connection fails before the supervisor replies,
Farhelm can reuse the approval on its next connection. If a takeover changed the owner but the reply was lost, retrying
the same approval cannot displace a subsequent owner. An agent-initiated Add of an already-managed host receives a
refusal naming the owner's tag.

The connection protocol changed. Older hosts use the usual Update path. The ownership rule prevents accidentally adding
a host in two Farhelm installations. Local supervisor tools running under the same account retain access.

## Open questions and possible follow-ups

The remaining real-Mac check would verify that Take over reconnects the new Farhelm to the existing supervisor and
leaves its sessions running after the registry is lost. The test compiled and needs an owned Mac target and a matching
signed build to run.

## Pull requests

- The supervisor remembers one managing Farhelm and ends the previous owner's connections on takeover.
  [#4187](https://github.com/scode/farhelm/pull/4187/changes)
- Farhelm preserves the ownership refusal, blocks provisioning and accepts takeover only for the owner the user chose.
  [#4195](https://github.com/scode/farhelm/pull/4195/changes)
- Hosts and Add host show the owner, and the Add host question waits until it is visible before accepting Take over.
  [#4200](https://github.com/scode/farhelm/pull/4200/changes)

## Checks run, reused and skipped

### Run

- `cargo nextest run --lib` across `farhelm-proto`, `farhelm-supervisor`, `farhelm-helm` and `farhelm-gui` passed 51
  selected tests for admission, ownership storage, the failed-dial retry and provisioning refusals. Run
  `498a0f3c-3750-4060-bf40-0159abd881de`.
- `cargo nextest run -p farhelm-proto --lib` with the hello, handshake and protocol filters passed 11 tests.
  `cargo nextest run -p farhelm --test e2e` with the version-skew case and `agent_relay` passed 11 tests. Runs
  `d30e4958-ef1a-43ba-9f09-c1cdd609bd4d` and `5d2ed7c3-b070-4714-990c-7baf74b507bc`.
- `cargo nextest run -p farhelm-gui --lib` passed 99 tests selected from Add host, the prompt, agent Add approvals, the
  frozen row and the local updater. The focused `a_discovered_owner_requires_takeover_confirmation` test passed again
  after moving the consequence text into the question's body. Runs `7299895e-5745-49c1-868d-1521c5414ac1` and
  `bd7f3759-f58d-4097-8c5f-eaaa7dbf9dd8`.
- `python3 scripts/check-gui-goldens.py --update` captured 264 Linux pictures. The 260 existing references matched byte
  for byte. Both new scenes were inspected in light and dark, published, fetched and compared with the final captures.
  `python3 -B scripts/test-gui-screenshot-batch.py` passed all 47 tests. Runs `d149d254-939f-42a2-ba38-261d9da53123` and
  `473bd022-6e75-4085-971f-ed33f6bb163d`.
- `python3 -B scripts/gui-e2e-smoke.py --no-build` passed 49 steps, including host Setup, Update, Remove, Uninstall and
  an agent's request to add a host. The harness cleaned up the test app and its processes. Run
  `a9a5ae59-a062-4437-bfdb-dcf4083f9420`.
- The combined screenshot list check found that the recently landed covered-terminal-link picture was missing from the
  gallery. Added its entry.
  `python3 -B scripts/test-gui-screenshot-batch.py SceneList.test_the_committed_scene_list_is_well_formed` then passed
  against the combined manifest. Runs `f34ba6cf-1103-4e4f-8f2a-f9507f0b4b83` and `7fb0e1cd-072b-4fb7-b57b-482f950e6108`.
- `cargo clippy --locked --all-targets -- -D warnings` passed for the changed crates, and again for `farhelm-gui` after
  the layout change. `cargo fmt --all -- --check`, `git diff --check`, `python3 releasing/check-changelog.py format` and
  `python -B scripts/check-test-sleeps.py` passed.

### Reused

- A 620-test `cargo nextest run` on the first ownership working tree over `099c6958` still covers unchanged supervisor
  store, helm client and manager cases. The recorder identifies that working tree by fingerprint `989707f9bece`. The
  later selections cover the subsequent admission and provisioning changes. Run `07cc9b41-01b5-474d-ae99-418c940c3b6f`.
- The final rebase over `d1f86047` preserved the ownership patches, with the gallery entry as the only follow-up edit.
  Main gained separate terminal-link, demo-recording and checkout-hint behavior. The normal Hosts and confirmation
  surfaces still draw and behave as tested at stack tip `0e0fd50b`.

### Skipped

- The real-Mac recovery test needs an owned Mac target and a matching signed build, which were unavailable.
- The Linux smoke skips the large clipboard paste because headless Mutter does not give the app clipboard ownership.

## Review gate

The required independent GPT-6.2 Sol reviews at high effort cleared all three PRs after their findings were addressed.
The previous Farhelm's connection is retired before it can send further requests. A retry of an earlier takeover
approval cannot detach a later owner. Add host accepts approval only after the question has been displayed. The focused
tests cover these boundaries.

### Landing

#### What landed with it

It landed in a round of five plans: drawn-frame-actions, one-helm-per-supervisor, gui-ux-discards,
gui-ux-backend-test-gaps and gui-ux-gui-test-gaps. Since its base, main gained the previous rounds' plans (Shift+drag
marking, model family words, unusable-folder refusals and earlier ones) and bookkeeping. drawn-frame-actions, in this
round, changed the shared test helpers so a simulated key first shows the frame. That broke this plan's test that input
before the take-over question is shown cannot take over, which failed on the boats as the reviewers predicted. The two
diffs share no lines, so stacking reported no conflict.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. No compatibility problem with
older or newer supervisors and helms, and no path where two helms both believe they own a host: the owner record, the
durable write, ending the old owner's connections and registering the new one happen under one lock. Agents can never
take over. The real Podman environment scenario failed once on a loaded boat and passed when re-run alone (see Checks);
this plan's executor had not run it.

#### Fixes made while landing

- **The take-over test now sends its early keys raw,** so it again tests input that arrives before the question shows.
- **SPEC.md still said a brand-new helm silently re-registers an existing host.** That contradicted the new rule; it now
  says a host another helm owns is re-registered, and a running supervisor there adopted, only after the user confirms
  Take over.
- **SPEC_impl.md lost its design rule** against code whose purpose is to let a supervisor or session be shared between
  helms, which the next paragraph still refers to. It is back, with a note that Take over transfers a whole host rather
  than sharing it.
- **The handshake lost its explanation.** A comment explaining why every connecting peer sends its hello first had been
  deleted (the supervisor's reply may be a refusal, so it waits to read the hello). It is restored.
- **SPEC.md and SPEC_impl.md were unformatted,** which failed `dprint check`. Formatted.

#### Things you should know that the report does not say

- **Take over in a host's menu asks nothing.** On a frozen row reading "Managed by another Farhelm (…), connected now",
  one click takes the host over and disconnects the other Farhelm, while Add host asks first and warns about exactly
  that. The plan modelled the menu item on Adopt, which also acts at once, so I left it. Possible follow-up: have it ask
  the same question.
- **Devbuilds and the installed app now fight over remote hosts.** Adding a host to a devbuild asks Take over, and
  accepting it disconnects the installed app (and the reverse). `.agents/devbuild.md` still says remote hosts are
  shared, and the user docs (Add a remote host, Manage hosts) do not mention the new "Managed by another Farhelm" state.
- **An agent's add of a host another Farhelm owns gets advice it cannot follow:** "Use Take over in Hosts first", for a
  host that has no row in Hosts yet.
- **A remote Mac rebooted while managed elsewhere can get stuck:** Update, which starts its supervisor after a reboot,
  is refused until it is taken over, and Take over needs a running supervisor. Removing the host from this Farhelm,
  adding it again, running its setup and then confirming Take over gets out of it; removing forgets the host here only,
  and its sessions stay on it, as re-adding a host always re-registers its sessions.
- SPEC.md lost "Running several helms at once is unsupported in v1", and SPEC_impl.md's list of connection states lacks
  the new one. I left both; the first is arguably replaced by the new ownership rule, and the second is a small
  follow-up.

#### Checks

The five plans change the desktop app's GUI crate, the supervisor, the helm, the shared protocol, a patch to the
vendored GPUI Linux backend, the terminal emulator's tests, the brick-test harness's tests, the dependency lockfile and
the specs. The landing ran the lints and the dependency checks, the whole Rust workspace suite, the desktop process's
tests, the vendored GPUI core's tests, the brick-test harness's tests, the app's end-to-end smoke and the reference
screenshots (goldens). Builds ran locally; the workspace suite and the smoke ran on boat sandboxes (cloud VMs), from
binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `python3 -B scripts/test-gui-e2e-smoke.py`,
  `cargo-deny --locked check licenses sources`, `python3 -B scripts/check-crate-layering.py`,
  `python3 -B scripts/dependency-notices.py --check --only-if-inputs-changed-since origin/main`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` once without and once with `--features agent`: passed.
- `dprint check`: failed on SPEC.md and SPEC_impl.md, which one-helm-per-supervisor left unformatted. Fixed.
- The brick-test harness's tests (`python3 -B -m unittest discover -s releasing/mac-vm-test`): 277 passed
  (`2476ddca-d615-4ebe-8be6-cfbd14160787`).
- The vendored GPUI core's tests: passed (`1309823f-52a9-4d34-8f5e-4490021331b7`).
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`23619e63-3132-4115-a3de-1b8de2283a2d`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`1a1a1bf3-c564-4d63-bd5a-7f98ab136ff7`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`96f2b2cb-2797-4888-947c-a3d128988ac9`).
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  8068 tests, 4 failed (`5ffcbf0c-a1d7-46e1-8857-b06fcd598a8d`, `495fea20-f8ed-4fa3-9344-67ff5337a9fc`,
  `8ad794ef-8b95-4032-b700-e1f244bd2965`):
  - the macro compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat
    does not have, so it fails there every time; it was therefore not run this round, and nothing in the round touches
    it;
  - the Add host take-over test, broken by stacking these plans (see one-helm-per-supervisor's notes). Fixed;
  - the real Podman environment scenario, which ran past its 30-minute bound while sharing its boat with the rest of its
    shard;
  - a test that a scoped Stop leaves alone the jobs of a stock tmux server a session started, whose fixture found the
    pane job in the session's scope.

  The last two were re-run alone on boats on the same code and passed: the tmux test
  (`feaf27df-b63c-4a0b-acb6-8afb442eace0`) and the Podman scenario in about 11 minutes
  (`2ec35bdd-4a33-4e4d-9cff-07018e16c5d5`). Both failures look like load on the shared boat, not this round's code;
  their runs are kept. This is one observation of each, so neither is recorded as a flake.

On the fixed tree (one test's early keys, the specs' wording and formatting, and comments):

- `cargo fmt --all -- --check`, `dprint check`, `python3 scripts/check-agents-md.py` and
  `cargo clippy --locked -p farhelm-gui -p farhelm-proto -p farhelm --all-targets -- -D warnings`: passed.
- The Add host test module, locally: 40 passed, the take-over test included (`1db5cf13-ab3f-414c-9678-4e806893dd50`).

The rest was reused for the fixed tree: after it only that one test, the specs and comments changed.

Skipped:

- The Mac-only checks: no Mac-specific code changed.
