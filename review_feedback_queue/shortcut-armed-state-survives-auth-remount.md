# Armed shortcut state survives an auth/skew remount

Reviewed commit: 463cac1528b5c5e259404975380d321a3fff0412

## TLDR

A sequence started inside New can reappear saying Enter apply after New is gone. A complete credential and input
sequence turning that into a harmful launch remains unestablished; upgrade-filter coverage is only partial and
uncertain.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location:
`crates/farhelm-ui/assets/template-shortcuts.js:46-92,181-185,236-289; crates/farhelm-ui/src/auth.rs:447-489`. Locations
and revisions in the assessment below identify the reviewed source; later main is outside this review's pinned range.

Reviewed 463cac1528b5c5e259404975380d321a3fff0412. Independently reconciled g271 synthesis/discovery with current
source. Shared credentials can become usable without reloading the old page: device_secret reads same-origin
localStorage per request (auth.rs:364-371; api.rs:1867-1870), another tab stores there before its own reload
(auth.rs:461-476), skewed token exchange returns before installing credentials (api.rs:1934-1945), and the old page's
auth/build-skew branch remounts the tree (lib.rs:1775-1784). Thus refused credentials are not a general refutation of
f5; that part of the prior caveat is corrected. But g271's old-tab token submission does not preserve an armed launch
sequence through ordinary shipped gestures. Pointer/click/blur/hidden visibility cancels JS arming. Bare Enter while
armed is consumed, retires arming and reaches no missing sequence relay on TokenPrompt; a subsequent token submission is
already disarmed. Typing/pasting an unrelated key also cancels. AppBody-owned raw references and InLauncher can survive,
so a stale apply-worded indicator remains a conditional OTHER problem; raw state alone does not turn an idle Enter into
a launch. The background-sign-in-before-arming alternative has source ingredients (component-owned awaited exchange,
shared storage, durable credential cap), but this pass did not tie them to a complete source-produced ordering of
same-build refusal, later skew/remount and armed=true/armedHasEntry=true without cancellation. Narrow missing SOURCE
premise remains that complete surviving gesture/admission route, not whether stored credentials can be accepted. No
runtime-frequency claim substitutes for it. STOP INCOMPLETE; retain conditional OTHER f5, not verified highest and not
an empty pass. Stronger wrong apply-to-execution/user-work hypothesis remains visible for parent audit; no harmful
command/configuration manufactured. possible_cover: SPEC.md:3383-3393 token friction only and older-release FILTER as
possible partial cover; proposed_drop false for the whole finding, superseding the earlier compound-upgrade proposal.
Existing orphan-FIFO repair verdict and pending-successful-opening-label OTHER remain unchanged.

Provenance: `a031/f5`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.

## Additional review at d6645f77

Additional reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

Fresh source review preserves the earlier retained raw-state/remount mechanism but still finds no compatible ordinary
credential/input sequence admitting a harmful launch. Pointer, blur and unrelated-key cancellation remain relevant;
accepted shared credentials are not a blanket refutation. This is additional independent provenance for the existing
other-priority investigated uncertainty. No claim of a new runtime observation, highest consequence or blanket
upgrade-filter coverage is added.

Source and provenance, run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`:

- `g157/f8`: `crates/farhelm-ui/src/lib.rs:1758-1759,1863-1872; crates/farhelm-ui/src/list/view.rs:1138-1162`.
- `a031/g157-f8`: `crates/farhelm-ui/src/lib.rs:1758-1759,1863-1872`.
- `a049/f5`:
  `crates/farhelm-ui/src/lib.rs:1758-1759,1863-1872; crates/farhelm-ui/assets/template-shortcuts.js:166-194,259-268`.

Proposed drop: none for the retained remainder. No runtime reproduction, fix or triage decision was performed.
