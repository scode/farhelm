# Header Replace prompt never warns a live agent is killed

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Replacing a running session from its header kills the agent with only a generic prompt that never mentions anything
being killed.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F11 / COR-HEADER-REPLACE-NO-KILL-WARNING`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/src/session_view.rs:1682` — The header Replace confirmation never says a running agent (and its tabs)
will be killed

Replace creates a fresh session with the same settings and then deletes the source, which kills the source's agent, its
process tree and its tabs if they are alive. The sidebar's replace prompt and the interrupted card's replace prompt both
word their warning with `status::replace_consequence(&status)`. That function's contract is that a live status must say
the agent is killed and the conversation discarded ("still running — replacing kills the agent and discards the
conversation; …"), while Unknown admits uncertainty. The header's Replace prompt, the most prominent Replace button,
instead always shows the fixed text "replace this session with a fresh one?" (line 1682), whatever the status. A user
replacing a running session from the header gets no hint that anything will be killed.

SPEC's "with confirmation that says so when anything is still alive" applies to Replace's delete as much as to Delete.
The suggested fix is to render `replace_consequence(&shown.status)` in the header prompt, extended with the tab clause
from F1.

Restater note: the finding says the header path "lost this in #928". Commit 5f2cddb (#928) introduced the header Replace
button with the generic text from the start; nothing earlier was removed. The defect stands, but it is a gap in a new
surface, not a regression.
