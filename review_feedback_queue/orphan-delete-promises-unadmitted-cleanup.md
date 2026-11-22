# A pre-admission orphan Delete timeout can promise an outcome that Refresh cannot recover

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Sandbox Delete can report cleanup in progress before the helm has accepted it, so a later admission failure never
appears when Settings refreshes.

## Details

**other.** Source inspection.

crates/farhelm-helm/src/tensorlake/orphans.rs:186–243,250–259,334–350; crates/farhelm-helm/src/sessions.rs:3811–3873;
crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:301–319,355–373,523–525,
1a53d9d35889d7dde96e31320cc1681dd013737d

If local admission outlasts the request deadline and ultimately fails, Settings shows the generic 504 promising cleanup
carries on and the list shows how it ends. A refresh cannot recover this late refusal. SPEC_impl.md:6914–6923 separates
cleanup that is already running at the deadline from a 10-second vendor-lookup 502 before anything is recorded.
SPEC.md:549–558 promises retained errors for unfinished cleanup. Source wraps the whole task with the generic 504
(orphans.rs:334–350; sessions.rs:3811–3837), including local checks before the Running entry is made
(orphans.rs:186–242). The Settings handler shows the immediate answer and rereads once
(tensorlake_sandboxes.rs:301–319,523–525). Late refusals are logged by log_late_failure with status and a reply excerpt
(sessions.rs:3842–3872). A sweep failure after admission also stores a Failed row and Settings shows its reason after
Refresh (orphans.rs:250–259; tensorlake_sandboxes.rs:355–373); an admission failure leaves no such row. This is separate
from the accepted manual refresh after admission (IC-08). The reviewer suggested other, confidence source-confirmed with
no timing frequency established. Proposed_drop no; identified.

At `66ad3aaa95ef03a2130e2d0194d58353912ae9da`, the provider lookup's own timeout begins after the saved account read.
That read waits for the store connection on the blocking pool (`crates/farhelm-helm/src/tensorlake/config.rs:114`,
`crates/farhelm-supervisor/src/db.rs:249`). The provider deadline is therefore insufficient to bound the entire
pre-admission phase. An unanswered lookup alone can return its own refusal after the outer request has already promised
continuing cleanup.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `implementation-contracts/IC-03`.

Supplementary evidence is retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5` as `tensorlake-inventory/f10`.
