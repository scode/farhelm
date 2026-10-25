# One uncertainty marker can hide an older attempt

Reviewed commit: b63c0be1f3dc87eccf52b26e53bc36de141672a5

## TLDR

A later uncertain launch overwrites the single visible marker while an earlier managed attempt remains retained. The
original duplicate-launch claim was refuted; a compatible pre-admission sequence producing the stronger harm remains
missing.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location:
`crates/farhelm-ui/src/list/create_form.rs:2752-2767,5253-5259,5682-5683 at 463cac1528b5c5e259404975380d321a3fff0412`.
Locations and revisions in the assessment below identify the reviewed source; later main is outside this review's pinned
range.

Reviewed origin b63c0be1f3dc87eccf52b26e53bc36de141672a5. Producer correction independently agrees with the prior
backend qualification: withdraw the original high claim that an accepted managed A reaches Guarded after B overwrites
its uncertainty marker. A known reservation is reconciled before YOLO. Single-marker overwrite and managed-A retention
remain source facts; retain at most conditional OTHER for a possible contract mismatch, not a demonstrated duplicate or
highest consequence. New static dispatch evidence: tip client.rs 2589-2638 registers/enqueues a CreateSession frame and
waits without an operation timeout; handle_create_session at service/handlers.rs 358-373 spawns an owned mutation before
acquiring intent/directory admission. Reconciliation also runs as an owned task (2714-2775, previously read) and core.rs
7288-7318 obtains intent then directory lock. Once A has claimed its intent, a same-key reconciliation cannot
legitimately inspect an intermediate no-reservation state: admission serializes them. Earlier possible gaps are
pre-enqueue or before the spawned original task first claims the intent. No explicit shipped wait/order demonstrating
that original A remains there while B and the A reconciliation complete was established. HTTP ownership (middleware.rs
51-74; lib.rs 2477-2494) and UI timeout/error handling allow a caller to lose a reply while owned work continues, but
alone do not prove the required overtaking sequence. A hypothetical unpolled-task schedule is not promoted to a
confirmed runtime mechanism. If A remains unknown and checkout config becomes stale, durable refusal under admission
settles that key; changing root alone therefore does not revive the original claim. Missing source premise remains a
concrete surviving A dispatch/admission ordering plus a distinct admissible guarded allocation. Confidence: conditional
OTHER only; incomplete source investigation, no runtime claims. possible_cover: SPEC.md 910-913 and completed ledger
18658-18673 describe the desired guard but do not accept this variant. proposed_drop: no drop of the conditional marker
concern; original high duplicate sequence withdrawn as source-unestablished, preserved here and in prior events.
Separate a017 post-redraw ambiguity is outside this narrow verdict.

Provenance: `a032/f6`, `g240/f2`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
