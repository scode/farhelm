# Abandoned upload admission retains request bookkeeping

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Abandoning an upload while its host connection is temporarily congested can leave bookkeeping in the helm until that
connection ends. Repeated cancellations can accumulate entries.

## Details

Cancellation can suppress `UploadStarted` or the staging refusal in
`crates/farhelm-supervisor/src/service/uploads.rs:307–325,434–482`. Cancellation cleanup answers queued commits, but
neither task completion nor tombstone pruning guarantees another admission reply. Client abandonment suffices; session
deletion is unnecessary (`service/handlers.rs:2406–2421`).

The helm's upload guard removes its upload route without removing the pending admission request
(`crates/farhelm-helm/src/client.rs:3922–3957`). A reply or connection failure clears the pending entry (`:1822,1614`),
but temporary queue saturation need not end the connection
(`crates/farhelm-supervisor/src/service/connection.rs:920–960`).

Retire the upload's admission request when its guard is abandoned, following the existing download-specific ownership
pattern. Preserve cancellable supervisor sends. A focused regression should cancel admission while its response queue is
full, release congestion, and assert that no pending entry remains.

Suggested bucket: **other**. Confidence: **confirmed static mechanism**, corroborated at both ends of the connection.
Frequency, retained memory and material UX impact are unmeasured. No security, data-loss or availability consequence was
established.

Possible cover: the ledger's `upload-cancellation-drops-final-reply.md:12847–12858` discards a hanging upload into a
session being deleted. This remainder concerns client abandonment without deletion and cumulative bookkeeping.
`send-upload-ignores-cancellation.md:4286–4304` deliberately makes replies cancellable; it does not require keeping
abandoned helm entries. `download-cancel-retains-pending-admission.md:15296–15325` covers the analogous download phase
and explicitly limits that fix to downloads. SPEC_impl.md's Supervisor metadata retention and nonresponse exception
excludes accumulation during ordinary behavior by a correctly behaving supervisor. No exact Planned, BUGS, queue or
filter coverage was found for the remainder.

Proposed drop: none; preserve those related decisions for triage. Provenance: persistent Astra/high review,
`helm-wire-client-1/f1` and `supervisor-file-transfers-1/f1`.
