# Delayed host confirmations can disable another open dialog

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Opening app Settings, a help dialog, Add host or Remove while a host plan is loading can leave both dialogs unresponsive
to clicks and normal keyboard interaction when the host confirmation arrives. Escape provides recovery.

## Details

Host planning does not own the page operation lock while awaiting the plan and publishes the returned confirmation
without checking other modal owners (`crates/farhelm-ui/src/provisioning.rs:2000–2080,2802–2887`). Host Settings checks
`confirmation_may_open` (`hosts.rs:1834–1844`), but Add (`1670–1681`), Remove (`1859–1864`), AppBar's Settings,
feedback, SSH help and welcome entries (`app_bar.rs:716,735,739,742`), and the trust-hint Settings entry
(`claude_trust.rs:178,192–199`) do not share that exclusion. Each independent dialog makes the other's ancestor inert
(`modal_isolation.rs:126–140`); observing new siblings does not transfer isolation ownership.

Start an uninstall plan, open one of those dialogs before its reply, and let the plan return. A slow planning request
allows this without a sub-second gesture. Coordinate publication of delayed confirmations with all modal owners, or
apply the pending-confirmation exclusion to every conflicting entry point.

The document-level Escape handler programmatically clicks an enabled cancellation control
(`modal_isolation.rs:142–155`), and release removes the inert attributes it owns (`170–182`). This is not a permanent
lockup. Exact browser dispatch order and the number of Escape presses were not reproduced. Intentional Add host → SSH
help nesting releases the parent's isolation first and is not this bug. Automatic welcome is another unguarded owner,
but the manually opened dialogs suffice for the trigger.

Suggested bucket: **other**, following the preferences reviewer's final downgrade after tracing Escape recovery. The
host-management reviewer suggested **high** because the dialog workflow is blocked; that severity disagreement is
retained for triage. Confidence: **likely**, from static lifecycle and isolation tracing; no browser reproduction. No
unintended operation, lasting outage, security consequence or loss of work is established.

Possible cover: `TRIAGE_OUTCOMES.md`, `delayed-provisioning-confirmations-make-two-dialogs-mutually.md`, records a
completed fix for Host Settings; `SPEC_impl.md:594–596` explicitly describes that narrow guard. The remaining app-wide,
Add and Remove entry points are outside that implemented scope. `SPEC.md:2955–2958` accepts actions delivered to a
covered dialog, not mutual inertness. No matching Planned item, BUGS entry or current queue item was found.

Proposed drop: none. Provenance: persistent Astra/high review, `ui-preferences-and-navigation-1/f3` and
`ui-host-management-1/f4`; one combined underlying modal-coordination finding.
