# A failed launch makes an inherited host an explicit template choice

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain other. Pinning now goes through
`list/create_form/draft.rs:1663–1673`, which still sets `chosen_host=Some(id)` without a placement edit; Save as
template still treats that Some as explicit (`list/create_form.rs:9334–9335`). `SPEC.md:1160–1164` requires a hand pick
and offers default/inherited hosts unchecked; `SPEC_impl.md:225–228` distinguishes pinning from a placement edit. No
exact current coverage or later repair was found. The refactor preserves the same provenance mismatch.

## TLDR

After an ordinary launch fails, Save as template prechecks its inherited host even when the user never picked that host.
Saving without clearing the visible check can make a template installation-specific unexpectedly.

## Details

Bucket: **other**. Confidence: **confirmed by source**; original suggestion other. The Host check is visible and can be
cleared. No wrong-host launch, credential loss, security consequence or user-work loss is established.

Reviewed source change: `112fd9d812e95df7f69f66c85820d76faaf89582`, unchanged at the reviewed tip.
`crates/farhelm-ui/src/list/create_form.rs:6890` pins the submitted host into `chosen_host=Some` to keep navigation from
changing request routing. The default clone state remains non-Bound (`Unconfirmable`, 3447), so `ExplicitChoices.host`
at 9136 treats that bookkeeping as an explicit choice. `list/create_form/save_template.rs:90-123` prechecks and saves
Host from that provenance.

The failure returns the same mounted draft at 7086-7190. Reseeding does not remove the false provenance before Save as
template: `create_form.rs:4063-4072` reconciles source/host changes, but `draft.rs:2824-2901` clears `chosen_host` only
for a genuinely new `Some(prefill)` generation. Ordinary New has no prefill. Clone-host reconciliation at 2904-2927 also
requires one, and target retirement after 2930 preserves the draft. Clearing on a later fresh opening therefore does not
fix saving from the returned failure draft.

`SPEC.md:1141-1145` requires Host checked only after a hand pick and inherited defaults unchecked.
`SPEC_impl.md:220-223` calls submitted-host pinning owner bookkeeping, “rather than a placement edit”; 8928-8932
requires explicit choices separate from inherited placement. Preserve the request host while tracking whether the user
actually chose a host separately from that pin.

No exact current queue item, ledger decision, Planned TODO, unlanded plan, known bug or filter cover was established.
Generic live-draft submission and host/folder fallback items have different triggers and consequences. Future Podman
template work does not state a repair to this ordinary-host provenance.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `failure-host-template-provenance`; `g022/l3`.
Original route and failed-reset investigation remain in `prep/groups/g022/findings.jsonl`. No runtime reproduction ran.
