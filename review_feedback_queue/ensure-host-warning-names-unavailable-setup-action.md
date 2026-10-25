# An ensure-hosts warning names an unavailable setup action

Reviewed commit: ac2f81d4ab1b80ed409567a8f9d708dd4b53737a

## TLDR

The startup warning tells an ordinary remote host with an invalid install path to use a setup action its row does not
offer. Manual/API recovery remains possible; this is diagnostic correctness with ambiguous filter coverage.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-helm/src/ensure.rs:202-205`. Locations and revisions in the assessment below identify
the reviewed source; later main is outside this review's pinned range.

Reviewed commit: ac2f81d4ab1b80ed409567a8f9d708dd4b53737a. TLDR: Startup's warning about an unusable remote install path
now tells the operator to repair the registered host through its row's setup action, although an ordinary remote row
with no failed setup offers no setup action. The changed lines remove re-adding the destination as the alternative
advice, leaving the unavailable remedy. Source route: ensure.rs:147-179 emits the warning when an already-registered
destination's stored paths OR its ensure-file paths fail the full path rule. UI lib.rs:890-894 makes sets_up_locally
false for SSH. UI hosts.rs:924-929,2027-2033,2373-2380 passes this false local_setup to ProvisioningPanel. UI
provisioning.rs:171-181 only offers re-run for a failed recorded operation and automatic setup for local_setup, so a
remote host with no failed run has neither. Those exact menu predicates remain at pinned tip
463cac1528b5c5e259404975380d321a3fff0412. Update is a different action: with an absent discovery and a stored
nonabsolute binary path, backend service.rs:1122-1148 and provisioning/plan.rs:887-894 refuse it rather than repair the
path. Suggested bucket: other. Confidence: confirmed for the missing advertised action and diagnostic; runtime frequency
unmeasured, no source premise remains missing. Caveats: no security/user-work-loss route, no claim of a bricked
installation or that every manual/API recovery is impossible. At this intermediate revision the old Add dialog remains a
workaround; the next commit's duplicate refusal removes that particular shortcut. If only the ensure-file value is
wrong, correcting the file suffices and no stored repair is needed. possible_cover: FILTER.md 'Rare, self-correcting
glitches and imprecise diagnostics' may apply if this configuration trigger is judged rare; its rarity premise is
ambiguous for a user-maintained ensure file, so no filter exclusion is claimed. 'Supported host setup' and ledger
tilde-remote-paths-never-expand.md concern best-effort custom installs; tilde-in-remote-path-fields.md covers rejecting
new invalid paths, not this changed remedy. Pinned Planned is empty; BUGS, queue index and ledger searches found no
exact same diagnostic item. proposed_drop: none; parent may resolve the narrowly named filter boundary.

Provenance: `g281/f2`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
