# Restart prompt claims "still running" for Unknown

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Restarting a session whose status is still unknown shows a warning that the agent is running when Farhelm does not
actually know that.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F12 / COR-RESTART-PROMPT-UNKNOWN-STATUS`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/src/session_view.rs:1636` — The header restart confirmation says "still running" for an Unknown
status

The header Restart button asks for confirmation when `restart_needs_confirmation` is true, and that is the case for the
live statuses (Running, Waiting, Idle) and also for Unknown (line 59). The prompt text, however, is a fixed literal
(line 1636): "still running — restarting stops the agent and its whole process tree first:". So for an Unknown status,
and also if the status changes to Exited while the prompt is open, the prompt asserts something the UI does not know.
Unknown is common: a freshly created or just-restarted session reports Unknown by design until the supervisor has
evidence either way.

The delete and replace prompts handle this deliberately. `confirm_consequence` and `replace_consequence` word Unknown as
"status unknown — the agent may still be running…", and their docs cite SPEC's no-guessing rule. The restart prompt is
the one place that rounds Unknown up to "running". The practical harm is a misleading sentence right before a
process-tree kill, not a wrong action. The suggested fix is to derive the text from `shown.status` the way
`confirm_consequence` does.
