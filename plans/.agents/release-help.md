# Releasing a claim, and plan help

Two short flows: "release plan X", which frees a plan whose executor or monitor died or is stuck, and "plan help". Each
reads this before acting (`plans/AGENTS.md`, "Flows and reading lists").

## Releasing: "release plan X"

Only on the maintainer's request, for a plan whose executor or monitor died or is stuck: `queue release <slug>`. A plan
that was in flight goes back to `[pending]`, and whoever claims it next resumes it from its working log and open PRs
(`plans/.agents/executing.md`, "Executing one plan", step 7). A plan that was landing goes back to `[complete]`, and the
next landing round lands it, starting from whichever of its PRs have not merged yet; if all of them had merged, that
round only records the landing (the `complete, no open PRs, merged ...` flag in `plans/.agents/landing.md`, "A landing
round"). If the old executor or monitor is in fact still alive, its next claim check or claim-holder verb finds the
claim gone and it stops.

## Help: "plan help"

"Plan help", "help on plans", or similar means: summarize each flow in `plans/AGENTS.md` and `plans/.agents/` in one to
three lines, then stop. Change nothing. The flows: the todo planning flow; plan to implement TODO items; schedule triage
outcomes through the planning system; pick a plan to execute (and the in-order and named variants); drain the plans;
drain and keep monitoring; monitor for complete plans, and its one-round form, land the complete plans; review the
plans, including show (or check for) blocked plans; release plan X; abandoning, which happens inside review; and how
notifications reach the maintainer through ntfy.
