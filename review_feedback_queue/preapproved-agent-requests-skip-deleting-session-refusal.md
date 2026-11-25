# Preapproved agent requests skip the deleting-session refusal mark

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

While Farhelm is deleting a session (or tearing down a Replace source), a new request from that session which the host
already allows without asking is carried out instead of being refused, although the approval module's contract says new
requests from a session being deleted are refused until the delete finishes. The action is one the host's standing
permission already authorizes, so this grants no new authority — a contract deviation, not a privilege escalation.

## Details

**other.** Source inspection; commissioner confirmed the ordering at the cited lines. Consequence confined to the host's
standing grant: no cross-host reach, no new authority, no established loss of user-owned work. It would move up only if
a route showed it acting outside the standing grant or losing user-owned work.

In `crates/farhelm-helm/src/approvals.rs` `ask_with_answer`, the link preapproval return (lines 466-469,
`if allowed && state.link_opener.is_some() { return Ok(Allowed::Raw); }`) and the commands/own-sessions preapproval arm
(lines 475-480,
`_ if !direct_allow && (row.commands_without_asking || (row.own_sessions_without_asking && own_host_session_action)) => return Ok(Allowed::Raw)`)
both return before the deleting-session check, which is only reached on the card path (line 500,
`if inner.deleting.contains_key(&asking_key)` inside the lock). Preapproved requests never insert a pending entry, so
they never consult the mark. Contract: `Approvals::deny_session` (lines 248-260) — "Deny every request from `key`
waiting, and refuse new ones, for as long as the returned mark lives: the caller holds it across the delete it is about
to send" — and SPEC_impl.md "Permission prompts for agent actions" (the delete keeps refusing new ones until it
finishes). Reachable because a GUI delete waits on the session's agent-request fence, which a fresh preapproved request
can acquire first. The mark's stated purpose (no card outliving its session; the delete not held for a whole approval
wait) is not implicated by a request that neither cards nor waits, which is why this is a contract deviation rather than
a security hole.

Verify: hold `deny_session` for a session, send a preapproved verb from it, observe it acts. Fix shape: consult
`session_is_being_deleted` (approvals.rs:285) before the preapproval returns, or move the deleting check ahead of the
preapproval match.

Reviewer metadata (area 2 chunk C, approvals/guards): suggested bucket: other; confidence: confirmed (code deviation);
proposed_drop: none; possible_cover: none known. Commissioner validation: ordering confirmed at the cited lines;
coverage searched (queue, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no match.
