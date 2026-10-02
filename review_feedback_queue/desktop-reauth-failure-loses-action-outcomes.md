# Failed desktop reauthentication still discards in-flight actions without reporting their outcomes

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

If desktop sign-in recovery fails while an action is pending, the action can still finish on the server while its result
disappears without an unknown-outcome notice.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F6 / COR-DESKTOP-OUTCOME`, reviewer `correctness_data_flow`, pass 1. Confidence: **definite**. Review disposition:
**would surface**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-ui/src/auth.rs:235`. Recorded from the completed review without
rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: The
ledger entry `desktop-reauth-remount-loses-action.md` records the completed fix for the request triggering
reauthentication. This finding concerns concurrent actions and the failed-authentication render branch; that earlier fix
does not cover its stated scope. `desktop-reauth-failure-dead-end.md` covers the Retry control, not action outcomes.

A failed desktop sign-in attempt replaces the entire application with an authentication error and Retry button. That
removes the components which own tasks awaiting actions such as Delete. For example, a Delete can already be accepted
and cleaning up a remote session when an unrelated fleet refresh encounters token rotation and triggers webview
reauthentication. If that authentication fails, the Delete task disappears even though the server continues the
operation. The failure page retains neither its result nor a notice that its outcome is unknown.

Waiting for the request that triggered reauthentication does not protect other concurrent actions. It also waits only
for that request's response headers, not for the UI to decode and publish its result. Preserve accepted actions and
their outcome reporting through failed reauthentication, or explicitly identify each pending action whose outcome
becomes unknown before replacing the application. Keeping the application mounted beneath a blocking authentication
failure and retry surface may be the simplest approach. Test a pending Delete alongside a separate request that triggers
failed reauthentication. SPEC.md permits losing forms and drafts during sign-in recovery, but explicitly forbids
silently losing action outcomes.

Additional original anchor: `crates/farhelm-ui/src/list/view.rs:1708` (a component-owned Delete task).
