# A home bind inside the installation can expose retained files to uninstall

Reviewed commit: 0f6ad6e0d680a518b46bf43b50ff704877931216

## TLDR

A home bind mounted inside Farhelm's installation can let recursive uninstall reach files outside that installation,
even when kept data is registered using its ordinary home path. This is a conditional configuration, not observed
deletion. Its coverage by the deliberate-configuration and home-alias exceptions is unresolved.

## Details

Let real installation `L` be `H/.local/lib/farhelm`, with a non-recursive bind of home at `L/mirror`. Keep state at the
ordinary `H/.local/state/farhelm`, with no divergent mount at that suffix. Use the ordinary installed binary, matching
unit and registered installation, and no unfinished sessions or tabs.

`crates/farhelm-helm/src/provisioning/backend.rs:2483–2513` leaves both installation and state unchanged in the raw and
rewritten fields. `provisioning/plan.rs:795–822` admits the binary and finds no state prefix inside `L`; `850–853`
selects `L` for removal. `backend.rs:2674–2687` runs recursive removal without a mount-boundary guard. Descending into
the child bind can therefore reach home files outside `L`. A later cycle or busy-mount error need not undo earlier
removals. This does not claim every home file must be deleted.

Suggested bucket: **highest**, conditional loss of retained files. Confidence: **confirmed conditional source chain**;
no topology, recursive traversal or deletion was observed. This arrangement already evades the earlier textual check at
`1b05191054c6fd4014d2857f91d3fbf49a50aef6`, `plan.rs:790–812`. Neither home-normalization patch is claimed to introduce
it.

Possible cover: pinned-tip `SPEC.md:3528–3536` and `3181–3194`, and the exact decisions at
`TRIAGE_OUTCOMES.md:4099–4111,6738–6756,17567–17592`. The last decision repairs the case where state itself is named
through the mirror, putting its raw spelling under the installation. Here state lies outside both textual prefixes. The
report's low-severity/unsupported characterization is not an explicit acceptance in those coverage sources. No exact
Planned, BUGS, queue or filter match was found. Proposed drop: none; independent exclusion judgment was ambiguous.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, `a001/f6`, derived while independently auditing `g001/f1`. The original narrower
example is repaired; this variant is retained separately. No runtime reproduction.
