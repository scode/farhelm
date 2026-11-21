# Guarded Delete and Replace can terminate work after the main shell exits

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Delete or Replace can end surviving main-shell work after a confirmation that mentions only extra tabs or says nothing
is running.

## Details

**highest**. Confirmed by source and contract inspection.

Confirmed conditional source path, no runtime observation. SPEC.md:2103–2105 requires a Delete confirmation
`when anything is still alive`; :2097–2102 requires a Replace with warning while `the source has anything alive`;
:2128–2138 binds both to drawn liveness. GUI delete.rs:81–98 and :271–292 submits an ended tabless ordinary row under
NothingAlive without asking. Replace.rs:119–138 withholds the live-work warning under the same predicate. Ended rows
with tabs name those tabs and use AgentEnded without disclosing surviving main-shell jobs. Helm sessions.rs:4857–4896
and :5009 passes the Replace source guard to ordinary Delete. Supervisor service/core.rs:13353–13423 checks terminal
hosts and their direct children, not jobs in the dead leader's scope or marker fallback. Service/handlers.rs:1258–1280
hands the accepted request to teardown, and service/teardown.rs:531–539 sweeps `SweepTarget::WholeSession`. Cgroup
inheritance (scope.rs:6–15) establishes the same survivor condition as shell-runtime/f14's distinct Stop handler. Turn 5
adds the approval-card manifestation from shell-permissions/f15: agent_requests.rs:2817–2835 and :2844–2852 chooses
NothingAlive or AgentEnded and sends the identical guard on the fleet route. Approvals/model.rs:534–546 explicitly says
`nothing; refuse if the agent or any terminal tab is alive` or `terminal tabs; refuse if the agent is running again`.
SPEC.md:4182–4184 binds the route to its shown state, and SPEC_impl.md:7112–7116 says
`Without the flag, anything alive is refused`. The fleet Delete without the flag can still reach the sweep after an
ended tabless listing. This is a manifestation of the same guard gap, separate from shell-permissions/f15's live-shell
wording defect. Estimated highest for ending running work beyond the warning or an affirmative refusal promise. Selected
action authorizes removing stored state. The problem is the additional undisclosed termination. Possible_cover:
SPEC.md:2147–2149 permits ordinary teardown, without waiving drawn-liveness guarantees. Separate from
shell-presentation/f4's connected environment Delete copy, presentation/f12's stale consent, and contract/f5's
successful-create prerequisite. 05ae73f23eb2cbb7ab284ca42473d66a771207a9 corrected ordinary Delete's live wording. The
guarded routes here remain at pinned 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. Turn 6 confirms the same approval scope
inside a disposable environment, as shell-runtime/f21 notes. SPEC.md:4575–4589 applies ordinary authorization and
authorizes only removal of the selected session, never the environment. Agent_requests.rs:2825–2833 passes the same card
guard to delete_session_within_environment. Helm sessions.rs:4042–4097 joins the fleet and confined routes at the same
guarded delete_session_fenced call after a connection check. Add that manifestation to this shared guard finding. This
does not duplicate shell-presentation/f4, which is the separate user Delete that can seal the environment. No change to
the consequence or bucket.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-contract/f21`.
