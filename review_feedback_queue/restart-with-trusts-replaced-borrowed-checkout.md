# Restart with can trust a replacement checkout folder

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

If a session was created separately inside a managed checkout, Restart with can automatically trust a different folder
later placed at that checkout's path. The ordinary new-session path checks the folder's identity before granting that
trust; this restart path uses remembered membership instead.

## Details

Suggested bucket: provisionally **highest** for automatic repository-content trust. Confidence: **likely**. The path is
traced statically, but no runtime reproduction or harmful agent behavior was demonstrated. Severity remains unsettled:
replacing the folder requires local authority, and someone able to do that can already alter the real checkout's hooks.
This is a remaining identity-check inconsistency, not a claim of isolation from deliberate same-account interference.

With the relevant agent trust switch enabled, `crates/farhelm-helm/src/sessions.rs:2816–2830` decides whether Restart
with should grant trust from the session listing's `working_copy.is_some()`. The supervisor supplies that display
metadata from database membership and canonical pathname, without checking the present folder's identity
(`crates/farhelm-supervisor/src/store.rs:3133–3187`). The new-launch membership read does check the recorded identity
(`crates/farhelm-supervisor/src/working_copies.rs:1157–1186`).

The original session that created the checkout has another safeguard: its restart calls `recover_checkout_destination`
(`crates/farhelm-supervisor/src/service/core.rs:11065–11077`), including the identity comparison at 8758–8768. A session
created separately inside that checkout has membership but no such origin, so it skips this branch. Its later ordinary
restart check at 11219–11221 calls `ensure_cwd_identity` (3142–3170), which compares canonical pathname strings.
Replacing the directory at exactly the same path passes that comparison. The helm's trust-bearing replacement launch can
therefore reach the borrower restart despite the folder's changed filesystem identity.

Possible cover: `TRIAGE_OUTCOMES.md:15736–15762`, "reused-checkout-path-grants-trust.md", previously requested identity
verification before granting managed-checkout trust, with a complexity-gated fallback to the out-of-band manipulation
exception. Its execution says the identity check was implemented, rather than taking that fallback. Independent review
disagreed with dropping this remaining borrower path as accepted behavior. `SPEC.md:3527–3540` accepts some manipulation
outside Farhelm, and `3171–3191` excludes deliberate same-account interference; preserve those limits for triage. No
matching Planned item, BUGS entry or open queue item was found. The automatic trust consequence prevents applying a
transient-display filter.

To reproduce in an isolated fixture: create a managed checkout and an independent Codex session inside it, stop that
session, replace the checkout folder at the same path without changing its root, enable Codex checkout trust, and use
Restart with a valid named or resumable target. Inspect the resulting trust arguments. Compare an ordinary new launch
there, which should not receive managed trust, and a restart of the original checkout-creating session, which should
refuse the replacement. A possible narrow fix is to use identity-checked membership for the restart's trust decision,
rather than treating display metadata as authorization.

Provenance: persistent-review A16-trust/A16-F2, independently retained by drop-check-06. The finding is narrowed to
borrower sessions; the original-session identity guard is not claimed broken. No runtime tests were run.
