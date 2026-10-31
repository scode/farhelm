# Trash alias refresh skips movement detection

Reviewed commit: 2256461ae98bf084885914b4c6a36e684cc43886

## TLDR

A same-GUI host alias refresh can change trash layout without triggering its movement pause. A supported layout placing
another enabled Delete under the old pointer remains unestablished, so this is a conditional fingerprint gap.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/trash.rs:728`. Locations and revisions in the assessment below identify the
reviewed source; later main is outside this review's pinned range.

Reviewed commit 2256461ae98bf084885914b4c6a36e684cc43886. Synthesis independently verifies g227/g279's same-GUI delayed
alias-GET path: set_alias decodes and discards its Host response (api.rs:4180-4187,4438-4453); the host mutation
requests on_changed before dropping its guard (hosts.rs:1605-1623,1651-1658); on_changed calls request_hosts without
awaiting it (list/view.rs:1975-1978,4008-4020; reader.rs:644-679). Settings Close is independently available
(hosts/settings_dialog.rs:665-673; hosts.rs:1989-1995), and TrashControl shares the hosts signal
(list/view.rs:4072-4074). Thus an accepted host GET can publish after trash opens and settles. With stable host
identity/order/archives, the name changes neither trash read signature nor settle fingerprint
(trash.rs:477-486,708-741). Single-host behavior retains the same wrapping header (850-890). The separate ScrollRegion
observer only owns tabindex/focus, not settling (modal_dialog.rs:408-554). The regression added by this commit
explicitly constructs an inserted-row overlap and asserts elementFromPoint before clicking; its fixture does not
establish alias-only geometry. Aliases are bounded64 without controls (helm store.rs:1588-1606); CSS wrapping, native
overflow, centering and anchoring do not yield a concrete harmful-overlap inequality from the read source alone
(app.css:8164-8279). I cannot establish another enabled Delete hit area overlapping the intended one's old point under
an accepted alias/font/viewport/scroll combination. That missing SOURCE/layout premise stops this investigation
INCOMPLETE; it is not occurrence frequency. Bucket OTHER, confirmed fingerprint omission with possibly-highest
conditional consequence. Exclusion verdict DISAGREE for historical shifted-row repair or pre-redraw acceptance; same-GUI
route removes multi-GUI dependence. possible_cover: TRIAGE_OUTCOMES.md:18270-18288 only for historical tracked-row
movement, not this new alias remainder. proposed_drop: none for the remainder. No reproduction was run.

Provenance: `a056/f1`, `g227/f3`, `g143/f2`, `g279/f1`, `g280/f3`. These are reviewer-local identities, not additional
findings. Proposed drop: none for the retained remainder.

## Additional review at d6645f77

Additional reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

Idle viewport resize and browser zoom add a producer for the shared Trash movement-fingerprint omission. The inspected
state-driven settle fingerprint does not observe that geometry change, and the separate scroll-region observer owns
focusability rather than Delete settling. Source checks did not establish a supported complete pointer layout
transferring an enabled destructive answer under the old point. Suggested bucket remains other, unverified after
investigation for that harmful geometry; no destructive activation was reproduced. This does not revive a blanket
highest classification from the missing movement field alone.

Source and provenance, run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`:

- `g017/f1`: `crates/farhelm-ui/src/list/trash.rs:839-877,1050-1066`.
- `a020/g017-f1`: `crates/farhelm-ui/src/list/trash.rs:839-877,1050-1066`.

Proposed drop: none for the retained remainder. No runtime reproduction, fix or triage decision was performed.
