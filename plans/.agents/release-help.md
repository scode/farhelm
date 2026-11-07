# Releasing a claim, holding a plan, and plan help

Three short flows: "release plan X", which frees a plan whose executor or monitor died or is stuck; "hold plan X" and
"unhold plan X", which park planned work and schedule it again; and "plan help". Each reads this before acting
(`plans/AGENTS.md`, "Flows and reading lists").

## Releasing: "release plan X"

Only on the maintainer's request, for a plan whose executor or monitor died or is stuck: `queue release <slug>`, or
`queue release <slug> --hold` for an in-flight plan the maintainer wants parked rather than picked up again
(`plans/.agents/release-help.md`, "Holding: "hold plan X", "unhold plan X""). A plan that was in flight goes back to
`[pending]`, and whoever claims it next resumes it from its working log and open PRs (`plans/.agents/executing.md`,
"Executing one plan", step 7). A plan that was landing goes back to `[complete]`, and the next landing round lands it,
starting from whichever of its PRs have not merged yet; if all of them had merged, that round only records the landing
(the `complete, no open PRs, merged ...` flag in `plans/.agents/landing.md`, "A landing round"). If the old executor or
monitor is in fact still alive, its next claim check or claim-holder verb finds the claim gone and it stops.

## Holding: "hold plan X", "unhold plan X"

Only on the maintainer's request. Holding lets the maintainer plan work now and decide later when it is built
(`plans/.agents/queue.md`, "States, and the script that moves them"). "Hold plan X" (or "put X on hold") runs
`queue hold <slug>`, which works only on a `[pending]` plan. An in-flight plan the maintainer wants stopped and held
takes `queue release <slug> --hold`, a blocked one is answered with `queue answer <slug> --decision <file> --hold`, and
a follow-up on a landed plan is parked with `queue follow-up <slug> --decision <file> --hold`
(`plans/.agents/reviewing.md`, "Reviewing the plans: "review the plans""); never release, answer or follow up and then
hold, which leaves the plan pending long enough for an executor to claim it. A complete plan is finished work the
monitor lands, and cannot be held: say so and change nothing. "Unhold plan X" (or "take X off hold") runs
`queue unhold <slug>`, which puts it back to `[pending]` in the same place in `INDEX.md`, and idle executors wake for
it. Name several plans and each gets its own call. To plan new work straight onto hold, ask for it when planning
(`plans/.agents/planning.md`, "Planning: "plan to implement <TODO items>"").

Plans that run after a held plan wait while it is held. When a hold would stall such dependents, say so in the report
back, naming them.

## Help: "plan help"

"Plan help", "help on plans", or similar means: summarize each flow in `plans/AGENTS.md` and `plans/.agents/` in one to
three lines, then stop. Change nothing. The flows: the todo planning flow; plan to implement TODO items; schedule triage
outcomes through the planning system; pick a plan to execute (and the in-order and named variants); drain the plans;
drain and keep monitoring; monitor for complete plans, and its one-round form, land the complete plans; review the
plans, including show (or check for) blocked plans; release plan X; hold and unhold plan X, and planning onto hold;
abandoning, which happens inside review; and how notifications reach the maintainer through ntfy.
