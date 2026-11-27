# Report: say every agent-Delete cleanup notice reaches the notices bell

## What this was about

When an agent deletes a session with `farhelm agent delete`, the Delete can leave a cleanup warning: a managed checkout
it could not archive safely, or a Podman container the session started whose stop it could not confirm. Since #5088, the
helm puts that warning in the desktop app's notices bell as well as in the agent's answer, so you see it even if the
agent never passes it on. The helm posts every such warning. SPEC.md, SPEC_impl.md, the instructions Farhelm gives
agents, the code comments and the unreleased changelog entry all said only a leftover checkout went to the bell.

You decided to keep posting every warning and widen the wording to match. One PR (#5140) does that. Nothing Farhelm does
changes, apart from one sentence in the instructions agents get.

### What changed

- **SPEC.md** (Managed checkouts, the notices bell under Status, and `farhelm agent delete`) and **SPEC_impl.md** (the
  late-Delete notices, environment Delete, and the CLI's `farhelm agent delete`) now say an agent's Delete puts its
  Delete notice in the bell, whether it reports a checkout left behind or a container whose stop could not be confirmed.
- **The agent instructions** now say "Report any cleanup warning printed on stderr." instead of "Report any checkout
  notice printed on stderr." I chose "cleanup warning" over the plan's suggested "Delete notice", because an agent has
  no definition of "Delete notice".
- **Comments and messages:** the helm's comment and log line where it records the notice, an internal error message, the
  doc of the agent's Delete reply in the protocol crate, and two test doc comments.
- **The unreleased changelog entry** from #5088 now covers both kinds of warning.
- **TODO.md:** the entry asking for this alignment is removed.

## Things you should know

- **What the bell can show.** An agent's Delete always goes through the ordinary Delete, which archives checkouts and
  never destroys an environment, so its warning is one of those two kinds. The bell entry's title is a generic "Session
  deleted on <host>", so a container-only warning is not labelled as a checkout.
- **Left as they are:** two helpers for the app's own late Delete notices (not the agent's) still have "checkout" in
  their names, in `crates/farhelm-helm/src/sessions.rs` and `sessions/sandbox.rs`. Renaming them was optional and
  outside this wording change. The environment tests that mention a checkout notice keep that wording, because they test
  a checkout specifically.
- **A related sentence in the agent instructions.** The same instructions also say Restart only ever resumes the
  conversation. Once the Claude Restart change (claude-restart-before-first-turn, #5131, #5137, #5138) lands, a Claude
  session that saved nothing starts fresh instead, so that sentence will be slightly off. It belongs to neither plan, so
  I left it.

## Open questions and possible follow-ups

- **Update the agent instructions' Restart sentence** once the Claude Restart change lands, to say that a Claude session
  that saved nothing yet starts fresh. A one-line change; I recommend doing it then.
- **Rename the two late-Delete helpers** for consistency. Optional; I recommend leaving them unless that code changes
  anyway.

## PRs

- #5140: docs: say every agent-Delete cleanup notice reaches the notices bell (also removes the TODO entry).

## Checks

Run now:

- The agent-instruction tests, and the helm's notices and agent-request tests
  (`cargo nextest run -p farhelm -p farhelm-helm --lib --bins`, tests matching `agent_instructions`, `notices::` and
  `agent_requests::`): 176 of 176 passed (recorded run 9fb4eff2-2ce8-4405-93e7-885d055bfc53). After the review fixes
  changed the instruction sentence, the agent-instruction tests again: 10 of 10 passed (recorded run
  793a46af-1f3e-4e87-bd58-ae6cd114857c).
- `cargo clippy` with all targets for the helm and CLI crates, `cargo check` of the protocol and helm crates after the
  review fixes, `cargo fmt --check`, `dprint check` on the changed Markdown, and
  `python3 releasing/check-changelog.py format`, the last three re-run after the review fixes: clean.

Reused: none.

Skipped:

- Everything else. The change is wording in specs, comments, log text and one instruction sentence, with no behavior
  change; the tests above are the ones that read the changed text.

## Review gate

A fresh-context Opus 5.5 review at high effort, with the plan's charter and the test-authoring checklist, found nothing
high or medium. It checked against the code that the widened wording is true. It found two low problems: the protocol
crate still described the reply's notice as a checkout notice, and one SPEC_impl list read as complete. It also found
six nits: placement and wrapping, a doc that would go stale, the agent-facing term, the changelog's wording, and the two
helper names. All were fixed except the helper names, which are explained above.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the previous round: ntfy alerts for waiting and blocked agents
(with a new kind of notice in the bell for a blocked agent, and a limit of 500 such notices per host), Restart of a
Claude session before its first prompt starting Claude fresh, a recipe's Delete question staying open on Keep editing,
and an exited terminal tab's failed cleanup backing off. The limit on blocked-agent notices removes only blocked-agent
notices, so an agent Delete's cleanup warning is never pushed out of the bell by it. In the same round, one more landed
before this plan and two after: helm-replace-refuse-destination-change (the helm refuses a **Replace with** request that
changes folder or checkout), marked-right-click-cancel-focus (focus after cancelling a bulk Stop or Delete) and
stop-ambiguous-failed-restart-create (Stop's refusal wording). None touches Delete's notices.

An independent reviewer and I read the change and found nothing to fix.

#### Fixes made while landing

- **A TODO.md conflict.** The round's four plans each removed their own TODO entry, next to each other, so stacking them
  conflicted there. I removed all four entries; no code changed.

#### What the landing made untrue in the report

Nothing untrue, but the report's first follow-up is now due: it waited for the Claude Restart change, which landed in
the previous round (#5131, #5137, #5138).

#### Worth knowing

- **The agent instructions' Restart sentence is now wrong.** The instructions Farhelm gives agents still say that
  Restart only ever resumes the conversation. Since the Claude Restart change, a Claude session that has saved nothing
  yet starts fresh instead. Possible follow-up, recommended now: the one-line change the report describes.

#### Checks

Run once, on the round's four plans stacked on main as it stood when the landing started
(helm-replace-refuse-destination-change, agent-delete-notices-bell-alignment, marked-right-click-cancel-focus, then
stop-ambiguous-failed-restart-create), so each plan was tested together with the others and with the previous round's
changes, which none of the executors' own runs included. The four plans touch the helm, the supervisor, the command
line's end-to-end tests and the desktop app's session list, so the whole Rust test suite ran. The SSH test worker was
still unreachable, so the slow suites ran on boat sandboxes (x86_64 Ubuntu 24.04 VMs), and clippy ran locally:

- Locally, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent` (the build with the desktop app's
  test-automation hooks), all with `-D warnings`: clean.
- On three boats, every test of the workspace and the desktop app that runs by default (`cargo nextest run --workspace`,
  built once, split in three and run through the recorder), recorded runs 2021dc06-f8a9-499f-a932-5b205c06b17a,
  830feae8-2980-4f70-8213-1864c2a3a428 and ce9cbcb2-c7c1-45e1-bcea-70e2a57a9224: 9,186 passed, one failed, and no test
  that quietly passed for lack of systemd, Podman or SSH on the machine. The one failure is the test-trace macro's
  compile check, which builds a scratch crate offline from the machine's own download cache; a boat has no such cache,
  so that test always fails there, as `.agents/boat.md` records, and no plan in this round touches it. Run locally
  (`cargo nextest run -p farhelm-testtrace-macros --test compile_contract`), recorded run
  aa311d19-8ca8-4224-b6ca-8af001538521, it passed.
- On a fourth boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  5692442c-6eba-4f7a-84ea-a783f3917c82: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run. Its steps include the session row's menu, Replace, Stop, Delete and Mark read; none uses marked sessions.
- Locally, the full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run
  852c4855-d075-47b1-8cbb-dd08f75e9f93: passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown: passed.

Reused: nothing. Skipped: the opt-in tests, among them the real-agent drift tests, which spend vendor turns; the
installer and provisioning suites, since no plan in this round touches them; and any run on a Mac, since none was
available to the landing.
