# Image inspection can restore an unowned checkpoint workload

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Choosing a compatible local checkpoint image can restore its old workload and host mounts during account inspection,
then report preparation failed without owning the restored container.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Podman 4.9.3 dispatches a genuine checkpoint image to restoration before ordinary keeper isolation validation. Farhelm's
unnamed account inspector can reach that branch; the inspected startup/account checks do not reject a root operating
user before inspection. The route requires a complete compatible standalone checkpoint, free old identity, available
base/rootfs and mounts, and compatible CRIU/kernel/runtime. The inspector script does not produce its framed account
fields, so Farhelm refuses before arming normal keeper cleanup. Later cleanup names the derived session container, not
the restored identity. Helper-scope reaping may stop scoped processes; universal survival is not claimed, and it cannot
undo prior execution or remove the original identity. Rootless/refusal guarantees do not explicitly waive this
inspection side effect. No checkpoint was created/restored, no exploit example was developed, and deployment occurrence
remains unmeasured.

Source locations:
`crates/farhelm-supervisor/src/container/account.rs:315-368; crates/farhelm-supervisor/src/container.rs:602-619; crates/farhelm-supervisor/src/service/core.rs:15191-15227`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a050/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
