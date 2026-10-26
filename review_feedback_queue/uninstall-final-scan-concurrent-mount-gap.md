# Uninstall has a conditional post-scan mount gap

Reviewed commit: c25de467816648fac189ab6c4a56ca45dede924b

## TLDR

A mount introduced after uninstall's last containment scan could be traversed by removal. No complete supported
topology-change sequence establishing loss of unrelated work was found; the stable-layout repairs remain valid.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `crates/farhelm-helm/src/provisioning/backend.rs:2694-2700`. Locations and revisions in the assessment
below identify the reviewed source; later main is outside this review's pinned range.

Later-turn synthesis: independent exclusion verdict remains AMBIGUOUS; proposed_drop no. Reviewed
c25de467816648fac189ab6c4a56ca45dede924b. g258 synthesis withdraws its Farhelm-mount-producer requirement and explicitly
stops incomplete on whether an incidental post-scan mount/topology change is supported. The scan then recursive-removal
mechanism is established conditional on that input. Correct/clarify earlier priority wording: OTHER on established
evidence; possibly highest if a supported incidental sequence exposing user files is established. This missing
supported-input premise is not merely unmeasured runtime frequency. No acceptance established: same-shell scan
completion criteria and deliberate-setup/general alias exclusions do not settle the residual home-alias route. No
further history is invented; affected investigation remains incomplete at this source gap. Existing narrow g258/f2
newline and g260/f1 absent-leaf refutations do not remove this candidate.

The recording-time installer and macOS held-folder changes concern local app-bundle installation/uninstallation. They do
not supply the missing supported topology-change history for the separately reviewed remote provisioning removal path.
Stable existing mount checks remain source-established. Possible cover: the same-shell scan criteria and deliberate
setup exclusions above are partial only; none explicitly accepts the unsupported topology-change remainder.

Provenance: `a038/f1`, `g258/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
