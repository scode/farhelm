# Checkout root reopening lacks an identity comparison

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A file download reopens the checkout pathname after verifying it; a complete uncancelled delivery through replacement
remains unestablished.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Held-root descendant confinement is independently source-refuted and remains sound for the authentic opened root.
Ordinary Delete/New supplies pathname replacement, but GUI disposal, cancellation, Blob/native ownership and receiver
lifetimes leave the compatible uncancelled consequential-delivery premise missing. Source checks investigated those
boundaries. This is not a proven outside-file read or a reproduction of the separate token-path issue.

Source locations: `crates/farhelm-supervisor/src/service/downloads.rs:55-79,220-255,314-343`. Historical revision
qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d. Discovery pass 1 strengthens the existing f1 producer analysis; no
new issue/ID. Source basis: inspect_download retains the row's container launch, gets a verified checkout PathBuf, and
later opens it without a root-descriptor identity check. It holds no lifecycle or working-copy admission claim. Ordinary
last-reference Delete archives and retires that checkout (service/teardown.rs:738-766,816-819; store.rs:3453-3481;
working_copies.rs:2369-2382). Fresh naming ignores retired rows and permits a free old basename
(service/core.rs:2299-2313,8741-8789); allocation creates a new directory there (working_copies.rs:1428-1460). Therefore
Farhelm itself can replace the pathname with another checkout, without out-of-band host mutation. Suspected effect: an
old admitted request could open and return a regular same-relative-name file from the replacement checkout, outside the
original container's actual bind. Actual checks/results: download channels are connection-local, not session-owned
(channels.rs:25-36,73-92); BeginDownload spawns an independent task (connection.rs:634-650); Delete cancels uploads but
does not enumerate these download routes (teardown.rs:439-471). A cancelled request does not provide a bypass:
DownloadGuard Drop retires the route and sends AbortDownload (helm client/downloads.rs:137-153,220-226), and removing
DownloadRoute aborts its async task (supervisor downloads.rs:29-32; channels.rs:89-92). The spawned root-open syscall
may still finish after cancellation, but its result is discarded; this does not establish transfer of bytes. Browser
addon disposal aborts transfer controllers (terminal-files.js:656-669). Connection death drops all transfers
(connection.rs:683-706; channels.rs:157-163). Missing premises for a complete harmful shipped GUI history: the old
request must remain active and be delayed between successful registry observation and later root open long enough for
last-member Delete, reallocation and file preparation; the replacement must contain the same relative regular file; the
requester must receive it before cancellation/disposal; and a security/highest consequence beyond merely changed
same-path file contents must be shown. Asynchronous source boundaries allow the conditional ordering but do not
authenticate this practical schedule or UI/HTTP cancellation timing. Source-only restrictions prevent a
scheduling/cancellation reproduction; available source established the independent lifecycle and ordinary name reuse,
not an observed harmful sequence. Evidence that would settle it: a retained supported two-client trace or deterministic
regression exercising the real admission/deletion/allocation/cancellation boundaries, with both root identities and
returned replacement bytes. Suggested bucket: other for the established gap; highest remains conditional on a
consequential trust-boundary route. proposed_drop: only the original descendant-symlink escape, assuming the intended
root descriptor; do not drop the narrower root-reopen concern on that basis. possible_cover: SPEC_impl.md:2574-2581
requires actual checkout confinement while stating no hover reserves a read; that latter statement does not clearly
accept switching to a different allocation. SPEC.md:4053-4065 excludes out-of-band manipulation, whereas the explored
Delete/New producer is in-product. Coverage searches found no exact accepted/queued/planned counterpart; g119's
independently reported same interval is a parent-level duplicate, not coverage. No runtime evidence produced or
authenticated.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g118/f1`, `g119/f1`, `g020/f5`, `a002/f1`,
`a021/d1`, `a022/g119-f1`. These are original reviewer identities for this concern, not additional bugs. Full revisions,
withdrawals and independent verdicts remain in the private review evidence. No fix or triage decision was executed.
