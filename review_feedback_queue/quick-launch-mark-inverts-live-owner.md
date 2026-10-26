# A quick-launch mark click can reverse its shown action

Reviewed commit: 038e2f0eb6f561baaf5686a4af1af31f1b7b138f

## TLDR

A delayed mark/clear click chooses its direction from the current owner rather than the direction its render offered.
The conditional mismatch is established; no harmful automatic launch is shown, and delivery-window coverage is
ambiguous.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/templates.rs:2167`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Later-turn synthesis: retain one conditional OTHER intent-polarity record with provenance a047/f1, g274/f2, and g275/f2
(including g275 original inherited-live-handler lead). Reviewed commit 038e2f0eb6f561baaf5686a4af1af31f1b7b138f; also
present at 5fcc034722b6e24bce5e538c775fe11330fef277:crates/farhelm-ui/src/list/templates.rs:2130 and pinned tip
463cac1528b5c5e259404975380d321a3fff0412:2167. TLDR: delayed input can save mark instead of clear, or clear instead of
mark. Rendered row.anchor.marked supplies aria_pressed and tooltip (038:2165-2166); onclick captures name but computes
!lit from live preferences (2167). Binding read/recovery can replace ownership at 1637-1643 and 1673-1678 before busy
clears at 1694. A delayed event from the earlier lit-A row can therefore become Bolt(A,true) after recovery observes B;
fresh application at 1646-1653 writes A although the shown clear-A operation would leave B alone. The reverse unlit
variant is the same defect, not a separate finding. Producer, consumer, fresh-read write route and busy release are
source-established; exact render/event delivery occurrence remains unmeasured. No automatic launch,
wrong-machine/session operation, security harm, destructive-consent breach or work loss is established. possible_cover:
SPEC.md pinned 3439-3442 general pre-redraw delivered-action allowance versus explicit shown intent under SPEC_impl.md
786-797 remains AMBIGUOUS, not settled acceptance. The failed-settings-save plan does not specifically cover intent
capture. proposed_drop: none. DISAGREE with exclusion based on inherited lines; AMBIGUOUS on general pre-redraw
acceptance, so retain conditionally. Separate failed-rename mark display exclusion a047/f2 (g274/f1) remains narrow and
unchanged.

Provenance: `a047/f1`, `g272/f1`, `g274/f2`, `g275/f2`. These are reviewer-local identities, not additional findings.
Proposed drop: none for the retained remainder.
