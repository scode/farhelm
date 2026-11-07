# Fresh-checkout Launch silently skips Resume without an install identity

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

After interrupted initial sandbox setup, a suspended host can appear as an available fresh-checkout target, yet Launch
does nothing and shows no refusal. The new pre-resume identity requirement returns before even resuming the sandbox.

## Details

Source coordinates from the original records: `crates/farhelm-ui/src/list/create_form.rs:5332-5340,6464-6471`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Discovery verification completed; new issue, not one of the peers' known findings. NewTensorlakeHost registration at
store.rs:6598-6641 omits host_identity; the hosts schema permits its NULL default at 2528-2534.
provisioning/tensorlake_add.rs:486-503 commits installation coordinates and releases admission before execute_actions
finishes, so an installation action can fail before the first supervisor handshake has populated identity.
execute_tensorlake_add:355-381 preserves that row and releases its hold when the coordinates are present. The user can
then explicitly Suspend this unreachable Tensorlake host: lifecycle.rs:49-84 requires its binding/API key and
provisioning lock, not a known supervisor identity, and126-140 publishes Suspended. SPEC.md:276-277 expressly offers
Suspend while unreachable. HostView:518 reads the still-absent identity, while shared.rs:676-686 preserves both
Suspended and identity None.

Open New, select that host and a GitHub repo, and supply otherwise complete launch inputs (for example the Command tab
with a simple command and explicit non-YOLO assertion; no host model catalog is needed). shared.rs:398-400 treats the
suspended host as available; create_form.rs:4058-4062 treats its fresh repo as destination-ready even though no preview
authority exists. The new suspended-checkout branch calls checkout_preview_authority at 5332-5338; its
host.identity.clone()? at 924 fails, and 5339-5340 returns without setting an error or calling Resume at5369.
launch_refusal's prerequisites have no identity condition (190-221,4064-4083), so after error was cleared at5171-5172
the footer at6464-6471 has neither an error nor a prerequisite reason to display. The submit markup at8117-8130 remains
actionable.

The parent dcd6dba^:create_form.rs:5244-5284 resumes first without this identity precondition and reports either its
connection error or the renewed-preview boundary. The new helper can safely require identity before allocating a
checkout, but it must not prevent explicit resume or silently omit the refusal/remedy. Either resume then bind the ready
install's identity/preview, or tell the user that setup/identity must be repaired. SPEC.md:282-284 supplies
launch-on-suspended behavior and1204-1210 requires visible precondition refusals.

Suggested bucket: other, a silent no-op with a missing remedy in the interrupted-initial-setup case; no security,
process-loss or data-loss consequence shown. Confidence: confirmed through schema/registration, setup admission/failure,
lifecycle, host projection, enabled prerequisites and footer/handler source. No runtime experiment. proposed_drop:
false. possible_cover: none. Current error/identity failure is distinct from g006-l6's retained-key preview conflict and
g006-l1's custom-word authority omission.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g006/g006-d1`. The
retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their own
revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
