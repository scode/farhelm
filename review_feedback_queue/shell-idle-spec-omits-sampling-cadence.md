# The shell Idle timing description omits the per-host sampling delay

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

The shell description can understate how long a quiet shell takes to show Idle on a host with many sessions.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed documentation mismatch, bucket other, no measured timing. The follow-up says
`Idle after the screen has been still for a few seconds` (SPEC.md:5568–5571). SPEC_impl.md:3985–3993 explicitly counts
three comparisons per session and says sampling cadence grows with the fleet. Ticker.rs:167–202 documents the nominal
cadence and sets a two-second tick and a budget of 16; :1358–1370 samples only the budgeted entries, and
screen_reader.rs:238, :364–380 requires three unchanged comparisons. With 32 stable live sessions on one host, within
SPEC.md:2649–2652's scale of tens and below the cap, the nominal comparison interval is four seconds per session and the
quiet streak takes about 12 seconds after a sampled baseline, before work in each round. This is an illustrative source
derivation, not a time guarantee or a defect in the documented scheduler. The initial landed shell sentence gave no
seconds claim. Distinct from shell-runtime/f10 and shell-contract/f9, which concern visible output outside the sampled
region. Possible_cover: the implementation spec explains the delay but it is absent from the product's shell
description.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-contract/f12`.
