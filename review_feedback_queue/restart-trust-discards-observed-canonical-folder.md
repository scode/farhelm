# Restart trust drops the canonical folder observed by its lookup

Reviewed commit: 058b3b1945981a5e7ad6a706b9717ba5f08df81e

## TLDR

Restart with can carry managed-checkout trust into an ordinary folder if its alias briefly names a managed checkout,
then returns to the original folder before restart validation. No ordinary accidental producer of that alias sequence
was established. This is a conditional source-contract mismatch, not a verified accidental unauthorized grant.

## Details

Suggested bucket: **other** on established in-scope impact. Confidence: **confirmed conditional source mechanism**. An
in-scope trust-boundary route would move it to **highest**. The ordinary-producer premise is missing; runtime frequency
is separately unmeasured. Deliberate same-account interference has no demonstrated security-boundary impact.

`crates/farhelm-helm/src/client.rs:2920–2924` requires trust-granting callers to carry the canonical path returned by
ownership inspection. `sessions.rs:2833–2847` instead keeps only the membership boolean. Creation stores both the
submitted alias L and original canonical ordinary folder A (`crates/farhelm-supervisor/src/service/core.rs:8281–8312`).
If L names managed B during ownership inspection and returns to A before restart validation, B supplies the trust answer
while A passes the original-directory comparison (`3146–3174,11432–11440`). Static L-to-B redirection is refused; it
does not refute this two-observation sequence.

The accepted A reaches spawning (`core.rs:11624–11633,11808–11815,12148–12170`). Codex's trust marker is filled with the
actual launch directory (`14590–14595`; `agent_kind/mod.rs:2228–2258`), so the override is for A, not harmlessly bound
to B. The issue remains at review tip `98bb15850f5f5ba1df6ed66873338261c45ff948`; the reporter and hidden-trash changes
do not bind these observations. Tie any trust grant to the directory whose ownership supplied it.

Possible cover: tip `SPEC.md:3332–3355` excludes general deliberate filesystem interference but preserves specific
protections. `SPEC.md:1023–1033` withholds automatic trust from ordinary folders; `SPEC_impl.md:2481–2496` requires
canonical binding. The previous identity decisions (`TRIAGE_OUTCOMES.md:15736–15761,18784–18814`) do not expressly
settle this sequence. No exact Planned, BUGS or existing queue coverage was found. Proposed drop: none; independent
exclusion was ambiguous. Resolve that contract before broadening the implementation beyond this mismatch.

Provenance: trailing-review run `47edc805-f43c-4ef2-abb1-0bba27a41c2f/review-98bb1585`, `g044/f1`; independent auditor
`a009/f1` retained the conditional mismatch. `a001`'s static-alias refutation remains narrower.
