# Equivalent Replace controls keep separate YOLO questions

Reviewed commit: d1dfd00b203e4e6049fa4b24cf68c5775acfc101

## TLDR

Opening Replace from both the sidebar and session header can leave two YOLO questions for the same session. Cancelling
one leaves the other question actionable.

## Details

Suggested bucket: `other`. Confidence: likely; state, admission and rendering support the sequence, but simultaneous
questions were not exercised in a live UI. Execution still needs an affirmative click on the remaining question. No
security or data-loss consequence was established. Provenance: Astra high repeat review, independently checked by a
separate Astra high reviewer. Proposed drop: none.

Only the retry identity is shared. The sidebar owns `yolo_replace` at `crates/farhelm-ui/src/list/view.rs:2250–2258`,
while the header/interrupted card owns `replace_yolo` at `crates/farhelm-ui/src/session_view.rs:692`. Each opens its
local question after a YOLO refusal and releases admission (`view.rs:2377–2404`; `session_view.rs:1592–1610`). Admission
does not test the other control's question (`view.rs:838–855`). The shared confirmation component permits coexistence
and renders without modal isolation (`crates/farhelm-ui/src/yolo_confirm.rs:150–154,205–298`).

Cancelling clears only that local question and the shared key (`view.rs:4053–4060`; `session_view.rs:2724–2735`). The
other question's answer does not require its old key to remain current (`view.rs:541–564`; `session_view.rs:2675–2737`),
and continuation can mint a new key (`view.rs:2285–2336`). Thus a header refusal followed by a sidebar refusal can leave
two questions, with cancellation reflected in only one surface. Share the same-source pending confirmation as well as
its retry identity, or settle a narrower intended contract.

Possible cover: SPEC.md:765–769 expressly includes pending confirmation in the state shared by equivalent controls.
`TRIAGE_OUTCOMES.md`, “replace-retry-key-splits-across-controls.md”, states that principle but concentrates its
regressions on retry identity; it does not accept independently surviving questions. The persistent state is outside the
subsecond-race filter. No exact Planned, BUGS or existing queue coverage was found. Persists at pinned tip
afc828776596f2e6639d16d4203ccc8b3d0f5862. Static review only; no browser tests or historical runtime claims were reused.
