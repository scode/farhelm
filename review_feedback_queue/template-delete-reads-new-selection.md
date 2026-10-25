# Template Delete reads a different selection

Reviewed commit: b0e4eafe9fc2f2cae93e4cdbfbebc5824be4d783

## TLDR

On desktop, Delete still shown for one template can delete a newly selected template. Its temporary Undo does not
restore shortcut bindings; event frequency is unmeasured.

## Details

Suggested bucket: **highest**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/templates.rs:1816`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Historical reviewed commit b0e4eafe9fc2f2cae93e4cdbfbebc5824be4d783; survives at pinned tip
463cac1528b5c5e259404975380d321a3fff0412. TLDR: a Delete drawn for saved template A can delete B after B is selected,
clearing B’s saved shortcut bindings too. Historical references: templates.rs
1372–1380,1632–1670,1816–1824,1862–1868,2030,2113. Tip full ordinary source route: clean explicit-kind/no-trust A has
matching baseline/draft (templates.rs 797–807,1379–1387); row B at 2178 invokes depart 1743–1773 and changes previous
synchronously; old Delete 1956–1980 reads B; api.rs 1332–1347 only waits for outstanding preference writes and permits
the no-pending case; api.rs 3967–3978 sends unconditional name-only DELETE; farhelm-helm/src/templates.rs 157–208 uses
Precondition::None; store.rs 4752–4772 deletes that name and clears its shortcut bindings atomically. Render-captured
DeleteKind alone (templates.rs 1730–1736,2014) does not bind the target. Ordinary valid A/B receives a ten-second
client-only Undo (2027–2037; closes with dialog); Undo restores fields but does not restore shortcut preferences
(store.rs 4752–4755; GUI restore 2045–2086). Permanent legacy A can additionally suppress Undo for valid B if the
old-kind handler is used; this is an extra legacy-state case, not required for ordinary wrong-target deletion. Framework
source corroborates a conditional desktop schedule: pinned historical and tip Cargo.lock use Dioxus 0.7.10; matching
cached native.ts 353–408,566–590 sends an event synchronously, returning only response flags; desktop protocol.rs 58–64
and webview.rs 43–195 invoke Runtime::handle_event without rendering; core runtime.rs 359–380 explicitly permits
multiple events before rendering. Native rendering occurs separately in webview.rs 537–588, and JS applies edits in
requestAnimationFrame (native.ts 429–483). Thus there is no required redraw barrier between those two ordinary clicks.
Event timing/frequency on a concrete platform is unmeasured; version-matched cached source is not binary attestation.
Browser ordinary-task timing remains unproven and is not needed for the desktop conditional claim. No runtime test
performed. Suggested bucket highest for wrong-target loss of saved template; confidence confirmed conditional source
mechanism. Live target behavior partially predates assigned patch. Later in-range changes to this file were inspected
and do not repair it; tip remains affected. possible_cover: no exact queue/Planned/BUGS/ledger/queued-plan match.
SPEC_impl.md 786–797 forbids choosing a confirmed target from live state. SPEC.md 3439–3442 allows delivered subsecond
actions but preserves named guards; no clear permission to retarget. SPEC.md 3426–3432 accepts same-name template/Undo
overwrite, not selection-name retargeting. Ledger 12236–12249 is a save/reload guard, 13453–13467 concerns invalid
legacy Undo, 17086–17101 deliberate deletion and frozen screenshot wording. Those do not cover this trigger/consequence.
proposed_drop: none. Keep any proposed highest exclusion for independent audit. f2 is separate and still lacks a shipped
same-target valid-to-invalid transition.

Provenance: `g230/f1`, `g265/f3`, `a028/g230-f1-audit`, `a074/g230-f1-audit`. These are reviewer-local identities, not
additional findings. Proposed drop: none for the retained remainder.
