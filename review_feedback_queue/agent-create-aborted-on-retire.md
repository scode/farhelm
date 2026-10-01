# Agent-requested creates lose bookkeeping when the asking host reconnects

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

When an agent asks Farhelm to create or clone a session and its own host reconnects at the wrong moment, the session is
still created but never appears in launch history, and the agent gets no answer.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F22 / COR-AGENT-CREATE-ABORT`, tagged **definite**. Anchor and title: `crates/farhelm-helm/src/client.rs:1645`
— agent-relayed create and clone run on tasks that are aborted when the asking host's connection is retired.

Agents running inside Farhelm sessions can ask the helm to do things on their behalf through `farhelm agent …` (create a
session, clone one, stop, restart, rename, list). Those requests travel from the agent to its own supervisor, then up
the helm's connection to that supervisor. The helm answers each one on a background task tied to that connection
(`spawn_agent_answer` in `crates/farhelm-helm/src/client.rs`). When the connection is torn down or withdrawn by the
helm's connection manager (the host reconnects, its destination is retargeted, it is adopted, or the link dies),
`abort_agent_tasks` (`client.rs:1645-1654`) aborts every one of those tasks.

For create and clone, the task body is the same helm create routine the GUI uses (`agent_requests.rs:1145` and `:1305`
call `do_create_session`). That routine first asks the _target_ host's supervisor to create the session, over a
different connection, then does helm-side bookkeeping: caching the new session, recording it in the launch history that
feeds the create dialog's recent folders and suggestions, and so on. If the abort lands after the target supervisor has
been asked, the session is still created there, but the helm-side follow-up is cut off part-way, and the asking agent
gets no answer at all. The code's own comment acknowledges that the abort cancels "the ANSWER, not the act", and the
supervisor reports an "outcome unknown" ending to the agent, so a retry with the same intent key does not produce a
duplicate. What is lost permanently is the bookkeeping: that create never appears in launch history.

This is the same violation of SPEC_impl.md's "Who owns an accepted action" rule that F3 and F21 describe for GUI
requests, reached through a different entry point. That rule says once the helm accepts a state-changing request, the
work runs on a task the helm owns, and a client going away may only lose the reply, never part of the work. Practical
impact is low (a missing history entry). The suggested fix is to run the handler body for mutating verbs through the
helm's existing `run_owned` helper (`crates/farhelm-helm/src/lib.rs:1858`, which spawns the work as an independent task
and only waits on it), and keep only the half that sends the answer back abortable.
