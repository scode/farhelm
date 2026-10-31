# Container attachments use unreachable host paths

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Normal image/file paste can report success but insert a host path that the container's agent or tab cannot read.

## Details

Suggested bucket: **high**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Supervisor uploads publish under supervisor state (service/uploads.rs:922–954,1292–1324), while the container binds only
the checkout (container.rs:363–375). The helm returns that raw path and terminal.js:2268–2284 inserts it into the
agent/tab. Session view still enables the ordinary upload policy for container sessions. SPEC.md:2284–2319 and the guide
promise usable attachments; disabled conversation integration is not an attachment exception. The original and uploaded
bytes remain preserved. The source defect is confirmed; no upload or container session ran during review.

Source locations:
`crates/farhelm-supervisor/src/container.rs:363-375; service/uploads.rs:922-954,1292-1324; crates/farhelm-ui/assets/terminal.js:2268-2284`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis at reviewed d6645f771bee8ed03a66486ef8755834937d759d confirms f4 as a distinct high correctness finding.
Additional supported-consumer check: session_view.rs:1733-1751 passes the same session-only attach_policy into
farhelmTerm.sync alongside containerized specs, so the flag does not omit upload policy. Helm uploads.rs:351-355 returns
upload.commit()'s raw path as JSON; terminal.js:2280-2283 inserts that exact path into the still-connected agent or tab
input. The backend stage_claimed checks existence only and allocates under supervisor state at
service/uploads.rs:922-954; neither helm/supervisor upload handlers nor UI attachments policy contains a container
exception. SPEC.md:2284-2319 promises usable attachments for any agent/tab terminal, while the sandbox section:3622-3632
excludes supervisor state. Re-read the complete container user guide's Start/Working/Stop/limitations sections:78-142:
no attachment omission or disabled-paste caveat is stated. Conversation-integration off at docs:117-121 is narrowly
hooks/capture/$farhelm/browser, not file/image paste. No exact covering exception/pending fix found in both specs, TODO
Planned empty, unlanded plans, BUGS, queue index and ledger searches; SPEC_impl.md:3776-3787 still specifies host-state
attachment publication. Original local file and uploaded bytes are preserved; established harm is normal attachment
input unusable, no highest consequence. Confidence confirmed by source, practical runtime not exercised, proposed_drop
false, possible_cover none found. Parent interface question remains: preserve the attachment
storage/no-workspace-mutation and container-isolation contracts together, rather than exposing all supervisor state. No
fixes or tests run.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g019/f4`, `g086/f4`, `g088/f2`, `a024/g019-f4`.
These are original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and
independent verdicts remain in the private review evidence. No fix or triage decision was executed.
