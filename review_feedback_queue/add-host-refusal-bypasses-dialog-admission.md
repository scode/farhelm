# A delayed Add-host refusal bypasses dialog admission

Reviewed commit: a0013a0cfc5aaf839450a4f08fa696ff4a675573

## TLDR

Opening Settings while confirmed host setup waits can leave both Settings and the reopened Add-host form ignoring
ordinary input after setup refuses. Escape supplies recovery; browser dispatch remains unverified.

## Details

`crates/farhelm-ui/src/hosts.rs:1833–1840` closes Add before awaiting setup. Definite refusal reopens it unconditionally
(`1845–1852`), mounted outside the new admission wrapper (`1973–1997`). Settings stays available during the wait
(`app_bar.rs:709–719,778–780`). Non-success setup responses are definite refusals (`api.rs:4247–4262`). Each surface
installs isolation: Add via `hosts.rs:4354–4366` and `hosts/settings_dialog.rs:184–190,271–303`; Settings at
`settings.rs:114–128`. The Settings owner has made HostsPanel inert; reopened Add then makes Settings inert through
`modal_isolation.rs:164–180,198–207`. Its own focus destination remains under the first inert ancestor.

Preserve the refused destination/reason while deferring the reopened form or deliberately handing off the active modal.
Programmatic Escape cancellation (`modal_isolation.rs:183–195`) is a recovery path. This is an existing publication path
omitted by the fix and qualifies `releasing/changelog.d/host-dialog-admission.md:5–6`, rather than a newly added Add
flow.

Suggested bucket: **other**, consistent with the older modal lockout finding. Confidence: **likely**, with static
reachability/isolation conflict established but browser behavior/frequency unmeasured. No unintended operation or
user-work loss was established. Proposed drop: none.

Possible cover: final `SPEC_impl.md:668–681` requires reopening Add after refusal, not immediate mutual lockout.
`SPEC.md:3241–3244` accepts delivered actions behind another dialog, not mutually inert input. Ledger `12641–12653`
addresses the older Host Settings guard; `16881–16897` records the broader fix complete without this Add path. No exact
Planned, BUGS, queue or filter match was found. Provenance: `dialog-focus-contracts-1/f2`, discovery pass 1, trailing
review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `dialog-focus-contracts-1/f2`.
