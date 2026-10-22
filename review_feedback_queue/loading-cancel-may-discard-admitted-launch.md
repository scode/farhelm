# Loading Cancel may discard an admitted launch's recovery

Reviewed commit: 74c980691fc87881e4602962844970e7b4e3fb94

## TLDR

A loading-screen Cancel lacks the operation guard used by ordinary launcher cancellation. If that handler can arrive
after launch admission, it could close the launcher while a create is accepted, losing its result and recovery identity;
another launch might duplicate it. No shipped input/render sequence establishing that delivery was found. This is an
unresolved lead, not a verified reachable failure.

## Details

`crates/farhelm-ui/src/list/create_form.rs:5629–5636` renders the unguarded loading Cancel. Its parent clears
`show_create`, removing the launcher (`list/view.rs:3903–3916`). Ordinary backdrop, Escape and Cancel instead consult
the live operation claim (`create_form.rs:5483,5548,5676`).

**Missing source premise:** delivery of this loading handler after admission. Submit refuses pending setup
(`create_form.rs:4657–4660`); loading has no Launch control (`5497–5500,5629–5648`); automatic preparation is hidden
(`2643–2649`) and waits for completed setup/readiness (`4344–4418`). The callable unguarded callback alone does not
prove that a trusted action can reach it in the required state. The investigation stops incomplete after focused source
checks.

Suggested bucket: **provisionally highest** only for possible loss of work through duplicated processes; that impact is
also unestablished. The producer suggested conditional high for duplication alone. Confidence: **conditional
consequence, source-incomplete post-admission reachability**. Do not relabel the missing route as merely unmeasured
frequency.

Possible cover: `SPEC.md:858–873` and closely delivered-input acceptance at `3352–3355` need reconciliation; no settled
exclusion was found in the required coverage sources. Proposed drop: none. This differs from permanent-answer Replace
Cancel: that route keeps a question mounted and clears a shared key while a captured continuation proceeds. Here the
loading callback removes the launcher, without that answer/save trigger. They need distinct reachability analysis.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, `a017/f17`, first noted separately by `g032` discovery. No runtime reproduction
or whole-item exclusion.
