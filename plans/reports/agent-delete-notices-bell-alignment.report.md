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
