# Container client detachment hides still-live inner work

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Detaching the container client can leave the agent running while Farhelm treats it as gone; Delete can then stop it
without the live-agent warning, and automatic tab cleanup can stop inner tab work.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The examined Podman 4.9.3/conmon 2.1.10/crun 1.14.1 stack permits the client to exit while its inner exec survives.
Farhelm's pane-based NothingAlive classification and later whole-container removal then disagree about live work.
Independent source checks also establish the automatic tab-reap consequence through the inspected kernel/Bash
terminal-hangup contracts. SPEC.md:3644–3646 accepts later removal of detached work, but its application to the original
still-running agent and automatic tab removal is ambiguous. This is a source-complete conditional route for the examined
versions, not authentication of installed binaries, a runtime detach test or a claim of every configuration's behavior.

Source locations: `crates/farhelm-supervisor/src/container.rs:316-325`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g020/f3`, `a024/g020-f3`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
