# New can inherit a remote folder on its fallback local host

Reviewed commit: 4ae55cc50dafcae6836abe835939ca79034cbdf7

## TLDR

A host refresh before New mounts can pair the selected remote session's raw folder with the fallback local host. This
contextual mechanism predates the reviewed patch; final launch still shows and fences its target, and delivery-window
coverage is ambiguous.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/create_form.rs:2666-2670`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit: 4ae55cc50dafcae6836abe835939ca79034cbdf7. Parent explicitly retains f5 as an OTHER contextual
pre-existing finding. TLDR: New can keep the selected remote session's folder while first opening on the fallback local
host. open_new validates against its captured hosts, saves only the raw cwd and schedules New (view.rs:3477-3505); an
already-ready ListView-owned host read may commit before that render (view.rs:1748-1825; reader.rs:644-679; locked
Dioxus core0.7.10 scheduler.rs:179-220). The first form render binds that raw inherited cwd to its current destination
(create_form.rs:2646-2670), starts its default nonpending (2828-2830), and first-target bookkeeping does not rearm it
(1195-1208). Confidence: confirmed static conditional mechanism; actual occurrence/frequency unmeasured. Current pinned
tip463cac1528b5c5e259404975380d321a3fff0412 retains raw handoff at view.rs:3525-3539,4204 and mount relabelling at
create_form.rs:2903-2907. Ownership caveat: the full mechanism predates this assigned patch, as
parent5c45a7a77af8fd04396dd858032a9fcb415ce31c view.rs:3474-3502, create_form.rs:2466-2476 and draft.rs:2664-2668
establish. It is not new assigned-patch discovery progress, and no earlier introduction is assigned here. Attribution
alone is not product acceptance; this event supersedes the prior withdrawal as directed by the parent while preserving
its ownership conclusion. No wrong-machine dispatch, security, exfiltration or user-work-loss claim. possible_cover:
generic delivered-input acceptance SPEC.md:3439-3442 and subsecond FILTER.md:116-135 remain ambiguous; neither is
established as full coverage. proposed_drop: none; retain contextually for parent ownership/coverage audit. Distinct
from f3 history-response provenance, f4 browse-fallback admission and a010/g018 canonicalized-history contract. No
reproduction performed.

Provenance: `g242/f5`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
