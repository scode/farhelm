# A refused template batch can leave its earlier edits

Reviewed commit: 4ae55cc50dafcae6836abe835939ca79034cbdf7

## TLDR

A host refresh during the shortcut batch's context wait can make projection and per-template validation use different
snapshots. Earlier draft edits can remain after a later template refuses; no automatic launch or user-work loss is
established.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/create_form.rs:4012-4053`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit 4ae55cc50dafcae6836abe835939ca79034cbdf7. TLDR: a refused template sequence can still change part of the
open launch draft. Before its asynchronous modal-context check the visible-launcher replay captures host/catalog
snapshots (4012-4015). After the await it validates the whole batch using those snapshots, then invokes a use_callback
for each template (4034-4053). That callback updates at every form render and independently validates each template
using its own action_hosts/catalog (3836-3853; draft.rs:2144-2151; verified locked dioxus-hooks 0.7.10
use_callback.rs:9-24). A registry refresh during the await can therefore make whole-batch validation pass using old
hosts while replay uses newer hosts. A first name-only template applies (draft.rs:2282-2291), then a later template
naming an installation now removed, replaced, or identity-mismatched fails (create_form.rs:606-629; proto
launcher.rs:842-855). The loop stops without rollback, preserving the applied prefix. An automatic host
identity-mismatch update is sufficient; this does not require another GUI to edit the registry. HostOption derives that
flag from HostPhase (shared.rs:623-644), and known_hosts excludes mismatches. The desktop await has a real
result-channel boundary: launch_sequence.rs:447-454 joins document eval; locked desktop document.rs:88-110 and
query.rs:45-96,198-209 send JS work and await its IPC return. Framework files individually match archive members;
hooks/desktop archive provenance was verified in the initial pass and dioxus-document 0.7.10 archive matches reviewed
Cargo.lock:1320-1323. No claim that web eval necessarily yields in the same way; desktop route suffices. The mechanism
persists at pinned tip 463cac1528b5c5e259404975380d321a3fff0412 create_form.rs:4450-4492. Product contracts require no
changes from a refused visible press/sequence: pinned SPEC.md:765-770,818-837 and SPEC_impl.md:5362-5369. Six coverage
sources searched for batch/sequence/template partial/atomic/snapshot/refusal: no exact queue item, BUGS entry, Planned
item or ledger acceptance. The restart-with-template-refusal plan concerns same-value held fields, not mixed registry
snapshots; queued-launch and refused-sequence snapshot ledger items concern binding-name capture/FIFO pairing, not
partial mutation after prevalidation. possible_cover none exact. FILTER rare-self-correcting/subsecond match is not
established: the applied prefix is draft state, not a briefly stale label, and this automatic-refresh variant does not
require a person's action inside the IPC window. Suggested bucket other; this is a visible draft application with Launch
still required, and scratch form input is not user-owned work. Confidence confirmed source mechanism conditional on a
relevant form render during the context query; frequency unmeasured. Caveats: no runtime reproduction; web-specific
yield path unverified; no wrong-machine launch, security impact or user-work loss established. proposed_drop none. This
is independent of retained g242/f3 response provenance and a010/g018 canonical-path history.

Provenance: `a030/d1`, `g264/f2`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
