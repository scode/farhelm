### What this was about

When an agent asked to add a remote host (`farhelm agent host add`), it could only ask for an ordinary SSH host. A host
you reach over Tailscale needed you to notice that and tick **Tailscale host** in the Add host dialog yourself, and the
agent had no way to see afterwards which of its hosts use Tailscale. You asked for agents to be able to request
Tailscale directly, and for an audit of other gaps between what the app and agents can do when adding or removing hosts.
During planning you settled the audit: add Tailscale to the agent's host add and show each host's Tailscale setting in
the agent's host listing; deliberately keep the safety boundaries closed (agents still cannot take over a host another
helm manages, uninstall Farhelm from a host, delete a Tensorlake sandbox or Podman container host, or change an existing
host's destination, Tailscale setting or permissions); and leave two other gaps out of scope (an agent adding a
standalone Tensorlake host, and agent access to the recovery actions after a failed add). You also asked that the user's
final choice reach the agent through the existing approval reply, with no new "wait for the dialog to finish" mechanism,
and that a destination Tailscale cannot use be refused before any card appears.

What was built: exactly that.

- `farhelm agent host add <destination> --tailscale` asks for the Tailscale host option.
- A destination Tailscale mode cannot use (an `ssh://` address, or one with a port) is refused at once, with the same
  guidance the Add host dialog gives, and no card appears.
- The approval card shows a Tailscale host line when the agent asked for it and says nothing about Tailscale otherwise,
  as it does for an optional alias.
- **Open add dialog** opens Add host with **Tailscale host** checked if the agent asked for it. You can still change it,
  and the list above the fields of what Add will approve now has a "Tailscale host" line (Yes or No) that follows the
  checkbox.
- The agent's confirmation names the destination you approved and adds "with Tailscale" exactly when you approved it as
  a Tailscale host, which may differ from what it asked.
- `farhelm agent hosts --json` shows each host's Tailscale setting. The plain table is unchanged.
- SPEC.md, SPEC_impl.md, the agent instructions and the "Approve what agents do" docs page describe it. The TODO entry
  is removed.

### Things you should know

The first two were decided during execution, the plan having left them open; you may want to revisit either. The rest
are for your information.

- **Your edit in the dialog is checked again.** If, in the dialog, you pair Tailscale with a destination it cannot use,
  the helm refuses that Add and the request keeps waiting, rather than trusting the app to have checked. Your Add host
  dialog's own check would normally catch it first. The alternative was to leave that check to the later reachability
  test only.
- **The plain hosts table has no Tailscale column.** The setting is in `--json` only, as planned for the listing. The
  alternative was a new column in the table.
- **The reply reports the approved add only.** If setup is then refused and you change the reopened dialog and press Add
  again, that is an ordinary add the agent is not told about. A changed destination already worked this way. The agent
  instructions tell agents to check the host listing before relying on a host, and the listing shows the Tailscale
  setting.
- **Protocol version bump.** Without it, an older helm would quietly add a Tailscale request as a plain SSH host, so
  every host's supervisor and the app update together, as with any protocol bump. The agent-blocked-notifications plan
  (waiting alerts and agent notifications), still being built, also bumps the protocol version; whichever lands second
  takes the next number when it is rebased.
- **The command list in `farhelm agent instructions` is laid out a little differently.** With the new option, the
  `host add` line no longer fits the width the short commands line up to, so their descriptions now start a little
  further left.
- **The release note.** The plan said no release note is needed, but repository rules require a changelog entry for a
  new feature, so the PR adds one that tells whoever writes the release notes they may drop it.

### Open questions and possible follow-ups

- Possible follow-up: the two gaps you left out of scope remain gaps between the app and agents: an agent cannot add a
  standalone Tensorlake host, and cannot use the recovery actions after a failed add. Plan them if you want them.

### The PRs

- #5126 `feat: let agents ask for a tailscale host on host add`: the whole change (request, checks, card and dialog,
  reply, listing flag, protocol bump, specs, docs, tests, reference screenshot, changelog entry, TODO removal).

### Checks

Run now, on the final change:

- `cargo clippy` on the protocol, helm and CLI crates (`-D warnings`): clean.
- Through the recorder (run a8179d17-3bf2-468a-8fb5-fd53b01cbcf6, 675 passed): the protocol crate's unit tests, the
  helm's agent request and approval tests, and the CLI's own tests and its agent command tests.
- The docs website build, which checks every link: clean.
- A full update of the test reference screenshots, which showed that only the agent's Add host dialog (light and dark)
  changed. It was inspected in both appearances, and its new reference picture was uploaded to the screenshot store the
  PR points at. That screenshot now shows the box checked and the "Tailscale host: Yes" line.
- After the change's last edit, a comment's line wrapping, nothing was re-run: it changes no code.

Earlier, before the review's fixes. Those fixes touched only comments, specs, the docs page, one help line, one
assertion removed from the helm's test of the Tailscale check (re-run above), and the screenshot reference above; none
touched the app's or the supervisor's code, so these runs still cover them:

- clippy on every crate the change touches: clean.
- Through the recorder, the targeted tests of each part:
  - b62f826f-fc7e-4468-b6a8-bf3c5f79174c: the protocol crate, 299 passed.
  - 028c14c4-47ca-4d7c-af82-a3e9830af6d6: the supervisor's agent request checks, 6 passed.
  - 6ae61d6b-d217-485f-9e31-8d943189c5ec: the helm's agent requests and approvals, 181 passed.
  - 16e73f28-200f-42e0-96c7-38b342266dd6: the CLI, 195 passed. Run c7ed64da-471f-4f0f-83fb-bf2a1f48d51a before it had
    one failure, caused by this change: the agent instructions layout test pinned the old alignment. The test was
    updated, so this was not a flaky test.
  - a55e0d2f-c5ff-47c4-b328-2d15d68d3d17: the app's approval card and Add host dialog tests, 140 passed.
- `cargo fmt`, the test-sleep check (no unannotated delays), `dprint check` on the changed Markdown, and the changelog
  format check.

Skipped:

- The workspace-wide Rust run and the end-to-end suites. The change adds one field along one request path, and every
  part of that path's own tests ran. Two end-to-end test files changed only to set the new field, and clippy compiles
  them.
- The desktop app's end-to-end smoke test, whose steps do not include an agent adding a host.

### Review gate

Reviewed by a fresh-context Opus 5.5 agent at high effort. It found no bugs in the code. It agreed with the two
execution-time decisions and with the rest of the design. Its findings were:

- the reference screenshot still had to be updated;
- two descriptions claimed the reply reported more than it does (the reopened-dialog case above);
- the option's help text left out the optional `login@`;
- the docs page did not mention the option;
- a comment's line wrapping;
- one test assertion that proved little, now removed.

All were fixed, and a verification pass by the same reviewer confirmed them, apart from one comment line it found still
split, which was then fixed.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the previous round's two terminal features: Shift-Cmd-click on a
file path copies its contents into the clipboard, and Cmd+Shift+V (Ctrl+Alt+V on Linux) uploads the clipboard's text as
a file. They touch the terminal only, not the approval cards, the Add host dialog or the helm's approvals, and their
SPEC.md and docs changes do not overlap this plan's. In the same round, after this plan, two more landed:
codex-fork-and-vscode-resume (Codex sessions that forked, or whose conversation was started through Codex's shared
background service, can now be restarted) and cli-effort-reject-flag-values (`--model` and `--effort` no longer take the
next option as their value). Neither touches host adding. This plan is the only one that raised the protocol version,
from 91 to 92, and every place that names the number agrees.

An independent reviewer and I read the change, with an eye on whether an agent can now cause anything you did not
approve. It cannot: the agent's request only fills in the card and the dialog's starting state, the card shows the
Tailscale request when the agent made one, the dialog always shows the choice as Yes or No, the add uses the checkbox as
you left it, and the helm checks that choice again when your answer arrives. I also looked at the refreshed reference
picture of the agent's Add host dialog: it lists "Tailscale host: Yes" among what Add will approve and starts with the
box checked.

#### Fixes made while landing

- **A TODO.md conflict.** The round's three plans each removed their own TODO entry, next to each other, so stacking
  them conflicted there. I removed all three entries; no code changed.

#### What the landing made untrue in the report

Nothing changed by the landing itself, but the landing's review found two sentences of the report that were wrong from
the start:

- **The Add host dialog does not check the Tailscale destination itself.** The report says that if you pair Tailscale
  with a destination it cannot use, "your Add host dialog's own check would normally catch it first". The dialog has no
  such check. The helm checks the pair twice: when your answer arrives, and again when the dialog starts contacting the
  host, before anything is sent to it. The first check keeps the request waiting so you can fix it, instead of telling
  the agent its add was accepted and then failing.
- **The end-to-end smoke does include an agent adding a host.** The report skipped that suite because "its steps do not
  include an agent adding a host". One step has an agent's `farhelm agent host add` card open the Add host dialog, whose
  Add answers the agent's request. The landing ran the smoke, and it passed (see Checks).

#### Worth knowing

- **`--alias` can swallow the new `--tailscale` option.** `farhelm agent host add dev --alias --tailscale` names the
  host `--tailscale` and asks for no Tailscale. This is not new behavior: the command line deliberately lets free-text
  options such as `--alias`, `--title` and `--host` take a separate value starting with `-`, so `--alias` already took
  whatever option followed it; `--tailscale` is simply one more option it can swallow. Nothing happens that you did not
  approve, since the card shows that alias and the dialog shows "Tailscale host: No". Possible follow-up: decide whether
  those options should refuse a separate value starting with `-`, as `--model` and `--effort` now do
  (cli-effort-reject-flag-values, landed in this round).
- **The next protocol bump moves to 93.** agent-blocked-notifications, still being built, also raises the protocol
  version; it now has to take 93 when it is rebased.

#### Checks

Run once, on the round's three plans stacked on main as it stood when the landing started (agent-host-tailscale-parity,
then codex-fork-and-vscode-resume, then cli-effort-reject-flag-values), so each plan's code was tested together with the
others' and with the previous round's terminal changes. Because agent-host-tailscale-parity raises the protocol version
and the three plans between them touch the helm, the supervisor, the command line and the desktop app, the whole Rust
test suite ran. The SSH test worker was still unreachable, so the slow suites ran on boat sandboxes (x86_64 Ubuntu 24.04
VMs), and clippy ran locally:

- Locally, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent` (the build with the desktop app's
  test-automation hooks), all with `-D warnings`: clean.
- On three boats, every test of the workspace and the desktop app (`cargo nextest run --workspace`, built once, split in
  three and run through the recorder), recorded runs 26acb5ca-a85a-4103-b668-59565a8874ce,
  5c9b7564-ebae-4866-bfbb-645723ef7e96 and bd019450-b9de-45cd-a860-5fcf904aa72b: 9,129 passed, one failed, and no test
  that quietly passed for lack of systemd, Podman or SSH on the machine. The one failure is the test-trace macro's
  compile check, which builds a scratch crate offline from the machine's own download cache. A boat has no such cache,
  so that test always fails there, as `.agents/boat.md` records, and none of the three plans touches it. Run locally
  (`cargo nextest run -p farhelm-testtrace-macros --test compile_contract`), recorded run
  0029d888-b065-4155-8e84-58a9fd7430b7, it passed.
- On a fourth boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  a8513ea8-59fd-45a2-84a9-1c23134391b3: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run. One of its steps has an agent's `farhelm agent host add` card (without `--tailscale`) open the Add host dialog,
  whose Add answers the agent's request; the new option itself is covered by the app's and the helm's own tests.
- Locally, the full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run
  b7d1edee-04d1-4f1b-8bfc-c51b72da2fe2, the test of the screenshot tooling's list of screens, recorded run
  619f638a-151e-45d7-a959-f641e4d1506d, and the workspace's documentation examples
  (`cargo test --doc --workspace --exclude farhelm-desktop`), recorded run 8ec704c8-a5e7-42a4-9737-2112dfb9f14b: passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format`, `dprint check` on the changed Markdown, and the docs website build
  (`bun run build`, which checks every link): passed.

Reused: nothing. Skipped: the opt-in real-agent and real-Podman tests, which no plan touches, and any run on a Mac,
since none was available to the landing.
