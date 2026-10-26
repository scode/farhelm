# Retained-path copying has an unresolved hidden-Unicode warning

Reviewed commit: 6151a828a3d87579f703fd807571f35687b1c0b8

## TLDR

A retained removal path can copy hidden Unicode without a warning. Control bytes and line breaks are rejected, and
validated/copied fields match; the warning requirement and harmful downstream paste remain unestablished.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-helm/src/provisioning/plan.rs:826,901 at 6151a828a3d87579f703fd807571f35687b1c0b8`.
Locations and revisions in the assessment below identify the reviewed source; later main is outside this review's pinned
range.

Targeted cross-group synthesis on g210/g210-discovery-1.result.md, f5 additional exclusion. Reviewed commit:
6151a828a3d87579f703fd807571f35687b1c0b8. AGREE with its narrow different-field validation-bypass refutation.
User-facing TLDR: the uninstall notice's copy button receives the same state-directory value the helm validated when
making the plan; no second unchecked path field was found on this route. Exact mapping: provisioning/plan.rs:826
validates facts.state_dir.named; :901 assigns facts.state_dir.named.clone() to ProvisioningPlan.state_dir.
provisioning/service.rs:1245-1264 retains and returns that plan. provisioning/http.rs:111-115 serializes plan as
ProvisioningPlan. UI api.rs:3650-3657,3694-3696 deserializes plan.state_dir into UninstallPlanFacts.state_dir;
:4326-4333 directly decodes UninstallPlan. UI peer.rs:78-88 makes PeerText a transparent String with raw() borrowing its
unchanged value. UI provisioning.rs:1564-1575 copies that raw field into PendingPlan.kept_state_dir; :3695-3715 takes
the displayed plan token and inserts kept_state_dir as UninstallNotice.state_dir. UI hosts.rs:1546-1551 moves the same
notice into the visible notice slot; :2286-2288 captures and copies notice.state_dir. No conversion from confirmation
text, canonical/raw inspection comparison fields, host home or a newly read current state supplies a different copy
value. This does not claim notice display proves successful execution: registration precedes POST, and absence promotes
the notice. Suggested bucket: potentially highest for the hypothetical validation-bypass allegation only; no actual
different-field defect established. Confidence: confirmed scoped source refutation. possible_cover: actual named-field
mapping, not behavior acceptance. proposed_drop: true only for the different-field bypass allegation. Existing a069/f1
stays open: AGREE on invalid UTF-8/control-character validation; AMBIGUOUS on the residual warning disposition. Retain
hidden Unicode outside char::is_control, ordinary other-window removal following a failed uninstall, and the
conditional/unshown harmful-paste route. No global downstream safety claim. Clipboard receipt, human reliance and
harmful command execution remain unverified; no reproduction or tests.

Provenance: `a069/f1`, `g210/f5`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
