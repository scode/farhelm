### What this was about

When an agent or user launches a session or saves a launch template from the command line (`farhelm agent create`,
`farhelm spawn`, `farhelm agent template create`), they can pick the model with `--model` and the reasoning effort with
`--effort`. If the value was left out, the command line took the next option as the value instead of refusing the
command. `--effort --yolo` started a custom agent with `--yolo` as its effort and without the YOLO setting the caller
meant. Built-in agents were safe, because the helm refuses an effort they do not know and any model that starts with a
hyphen, but custom agents had no such check. `--model` has worked this way since before the last stable release.
`--effort` started working this way after a recent change that let a custom agent's effort start with a hyphen
(`--effort -fast`); that change has shipped only in the 0.46.0 release candidates.

You decided:

- both options refuse a separate value that starts with a hyphen;
- a custom agent's hyphen-leading model or effort still works written with `=` (`--effort=-fast`);
- the custom-agent contract itself stays as it is.

What was built: exactly that. `--effort --yolo`, `--model --yolo`, `--effort -fast` and `--model -fast` are now refused
as usage errors, naming the option that has no value (or the unknown `-f`). `--effort=-fast` and `--model=-fast` still
reach the launch unchanged. The options' help text says so. SPEC_impl.md's command-line section is updated and the TODO
entry is removed.

### Things you should know

- **The release note.** Compared with the last stable release, only `--model` changes: `--effort` never took a separate
  hyphen-leading value in a stable release. So the earlier change's unreleased changelog entry (which advertised
  `--effort -fast`) was rewritten into a fix entry about `--model`, with a note for whoever writes the release notes
  that the `--effort` behavior from the release candidates is reverted. The plan expected no release note; because
  `--model` changes for stable users, the entry was kept as a fix. Anyone who used `--effort -fast` on a 0.46.0 release
  candidate needs `--effort=-fast` instead.
- **One narrow edge remains on the template commands.** When saving a template, a template's name may itself start with
  a hyphen, so before the name is given the command line still takes a separate hyphen-leading value that is not a known
  option (`farhelm agent template create --effort -fast mytemplate` saves `-fast` as the effort). A known option such as
  `--yolo` is refused there too, so the case this plan was about is closed everywhere. Changing how the template name is
  read was outside the plan; SPEC_impl.md records the edge.

### Open questions and possible follow-ups

None.

### The PRs

- #5129 `fix: stop --model and --effort from taking the next flag`: the change, its tests, SPEC_impl.md, the changelog
  entry and the TODO entry's removal.

### Checks

Run now, on the pushed change (main has moved since only by queue bookkeeping):

- `cargo clippy -p farhelm --all-targets -- -D warnings`: clean.
- Through the recorder (run b7b93e65-76d6-4ab7-9f1d-046cfc28f760, 195 passed): the command line's own tests and its
  agent command tests:

  ```
  cargo nextest run -p farhelm --bin farhelm --test agent_cli
  ```

  The new tests check that each of the four forms above is refused with the expected error on `agent create`, `spawn`
  and a template save, and that the `=` forms reach the launch unchanged.
- `cargo fmt`, the test-sleep check (no unannotated delays), `dprint check` on the changed Markdown, and the changelog
  format check.

An earlier run of the same selection before the review's fixes (46b790da-6c99-4e13-abf8-3846e36ce33a, 195 passed) is
superseded by the run above and not counted.

Skipped: the workspace-wide Rust run and the end-to-end suites, since the change is confined to how the command line
reads two options, which its own tests cover.

### Review gate

Reviewed by a fresh-context Opus 5.5 agent at high effort. It found no bugs and confirmed the command-line parser's
behavior in a small standalone test program. Its findings were:

- the changelog entry described `--effort` as changed, when compared with the last stable release only `--model` is;
- the test accepted either error kind for every case, so an unrelated parse failure could have passed it;
- several comments, the help text and a test description said more than the code does;
- the template-name edge above, which the spec now records.

All were fixed, and a verification pass by the same reviewer confirmed them.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the previous round's two terminal features (copying a file's
contents with Shift-Cmd-click, and pasting the clipboard's text as an uploaded file), which do not touch the command
line. In the same round, before this plan, codex-fork-and-vscode-resume landed (more Codex sessions can be restarted),
which does not either, and agent-host-tailscale-parity, which added a `--tailscale` option to `farhelm agent host add`.
The two changes edit the same command-line definitions but different commands, and they do not interact:
`host add dev --tailscale` reads as intended, and this plan's new help text does not change the command list
`farhelm agent instructions` prints, whose spacing the Tailscale plan changed and a test checks.

An independent reviewer and I read the change. It confirmed the report's premise: nothing downstream refused `--yolo` as
a custom agent's model or effort, so the old behavior did launch with it.

#### Fixes made while landing

- **A TODO.md conflict.** The round's three plans each removed their own TODO entry, next to each other, so stacking
  them conflicted there. I removed all three entries; no code changed.

#### What the landing made untrue in the report

Nothing I found. The landing's checks were wider than the report's: the whole Rust test suite ran, not only the command
line's tests.

#### Worth knowing

- **Other free-text options still take a following option as their value.** `--alias`, `--title` and `--host`
  deliberately accept a separate value starting with `-`, so `--title --yolo` takes `--yolo` as the title. That is not
  new, but the Tailscale plan, landed in this round, added `--tailscale` beside `--alias` on `host add`, which `--alias`
  can now swallow. Nothing happens that you did not approve, since the card and dialog show it. That plan's landing
  notes suggest deciding whether those options should follow `--model` and `--effort`.
- **The template-name edge stays as the report describes**: before a template's name, a separate value starting with a
  hyphen that is not a known option is still taken, because the name itself may start with a hyphen.

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
