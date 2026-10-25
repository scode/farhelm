# A Bun option value can resemble its launcher marker

Reviewed commit: 4d20f72954d9f794abaf3002e8baa9ee9675894e

## TLDR

The OMP classifier may treat a Bun option value of x as its launcher marker. Actual supported Bun parsing and a nested
reporter route remain missing, so no wrong-conversation execution is claimed.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `crates/farhelm-supervisor/src/agent_kind/omp.rs:70-79`. Locations and revisions in the assessment
below identify the reviewed source; later main is outside this review's pinned range.

Synthesis, same reviewed commit 4d20f72954d9f794abaf3002e8baa9ee9675894e. Retain conditional OTHER; the affected
investigation is incomplete/stuck. Farhelm's first-non-flag scan treats x in [bun,--cwd,x,entry] as the x subcommand,
but authoritative Bun parsing for a supported version is absent from the pinned repository and supplied run source
inventory. No network access is authorized, and no vendor was executed. Therefore even the vendor-level
misclassification premise remains unverified; do not present the --cwd example as confirmed. Independently, no shipped
nested interactive OMP path loading the current gated reporter and producing the required ancestry was established;
parent reports that this same premise remains unresolved in g278/a048. Do not invent a private child launch. The source
link from classification through durable provenance to launcher_at_pane and child acceptance remains as originally cited
(agent_kind/omp.rs:70-79; service/core/vendor/omp.rs:37-48,194-225; procs/omp.rs:290-309,336-403,429-440).
possible_cover remains ambiguous/not established: package-launch positional acceptance applies to actual bun x/npm, not
demonstrably this direct-entry option shape; no blanket other-harness exception applies to established
wrong-conversation harm. proposed_drop false; potentially highest only after both missing source premises are
established. Not deduplicated with the older-classification investigation: f1 concerns newly classified argv, while
g278/a048's stated route concerns retained older classification.

Provenance: `g277/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
