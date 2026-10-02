# Creating a session while Delete runs blocks terminal input for the entire host

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Creating a session during Delete can freeze input and other requests for every session on that host. This finding is
already covered by the Planned creation-dispatch work.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F11 / COR-CREATE-DISPATCH`, reviewer `correctness_systems`, pass 1. Confidence: **definite**. Review disposition:
**would surface**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-supervisor/src/service/handlers.rs:2860`. Recorded from the
completed review without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording:
Covered by TODO.md, Planned, "Keep session creation off the connection read loop", and the completed planning entry
`create-runs-inline-on-read-loop.md` in TRIAGE_OUTCOMES.md. Retained at the user's explicit request to record all 22
findings. This is additional review evidence for already-planned work, not a new implementation decision or a request to
triage that decision again.

The supervisor receives session-management requests and terminal traffic through one shared connection reader. An
interactive create runs directly inside that reader and waits to acquire the host-wide lock protecting working-directory
operations. Delete holds the same lock throughout teardown, including process-kill grace periods. Creating a session
while another session is being deleted therefore leaves the reader waiting for Delete instead of reading later
keystrokes, resize requests, and session-list requests for every other session.

This occurs during ordinary concurrent actions on a healthy host. SPEC.md allows Create to queue behind Delete, but
requires unrelated terminal traffic and listings to remain usable during teardown. Dispatch creation as tracked, bounded
work outside the reader, preserving the existing intent, directory, and lifecycle lock ordering and ownership of
accepted mutations. Admission itself must also leave the reader free. Test Delete held in its process-grace phase,
followed by Create on the same connection, and prove another session's input and a list request progress before Delete
is released.
