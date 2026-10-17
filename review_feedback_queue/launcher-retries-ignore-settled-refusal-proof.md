# Launcher retries keep replaying a repaired refusal

Reviewed commit: e870329ce9be321493a2fc03de4f3cc67549fd2f

## TLDR

After New, Clone or Replace with fails because its folder is missing, restoring the folder and retrying unchanged can
keep returning the original failure. Editing or reopening the launcher can recover, but the error does not explain that.

## Details

Suggested bucket: `high`. Confidence: confirmed static mechanism; no runtime reproduction. Provenance: Astra high repeat
review of the pinned 139-commit range, with a separate Astra high source check. An archived report supplied the initial
lead; both reviewers independently traced the product code. Proposed drop: none.

`crates/farhelm-ui/src/api.rs:2203–2210` reads only stale-connection and YOLO markers into `CreateRefusal`. Ordinary
creation (`:2151–2172`) and Replace with (`:2345–2370`) use this type. The launcher reuses the key for unchanged choices
(`crates/farhelm-ui/src/list/create_form.rs:4988–5006`) and clears it only for a stale connection in this failure path
(`:5261–5309`). Meanwhile the supervisor replays a permanent failed reservation before validating the restored folder
(`crates/farhelm-supervisor/src/service/core.rs:7316–7337,14425–14444`). The new definitive-refusal proof reaches HTTP
but is discarded by these clients. Plain Replace consumes it through its separate action-refusal path.

Carry the definitive no-create classification through the launcher refusal type and retire its key only on that proof.
Preserve keys for ambiguous outcomes and YOLO continuations. A focused regression should fail an ordinary folder launch,
restore the folder without changing the form, and retry through each affected launcher entry point. Managed-checkout
creation uses separate handling and is outside this finding.

Possible cover: `TRIAGE_OUTCOMES.md`, “replace-retries-replay-settled-create-failure.md”, explicitly covers plain
Replace through the sidebar and header, not New, Clone or Replace with. SPEC_impl.md's “Errors crossing levels of
abstraction” requires durable same-key refusal replay; minting a fresh key after definitive refusal preserves that
contract. Its lost-reply rules concern ambiguous outcomes. No exact Planned, BUGS, queue or filter coverage was found.
This is a surviving adjacent omission, not a claim that the reviewed commit introduced key retention. Persists at pinned
tip afc828776596f2e6639d16d4203ccc8b3d0f5862.
