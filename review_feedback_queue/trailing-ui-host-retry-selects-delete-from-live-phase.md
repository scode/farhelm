# Retry promises a connection attempt but retries environment destruction

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Hosts' Retry says 'Try the connection again' but can instead retry destroying a sandbox and all
files/processes still in it. Captured-render contract: HostRow:5008-5020 captures only id in onclick and always draws
that connection description; it carries no displayed operation discriminator. Parent on_retry:2337-2380 reads
hosts.peek(), computes deleting from the current HostPhase::Deleting { failure: Some(_) }, then chooses
retry_environment_delete or retry_host. The operation is therefore chosen from live state, not the render acted on. This
mismatch exists even without a delayed click when the current row already reports failed deletion, and a queued
reconnect Retry can change operation after its render. SPEC_impl.md GUI: Dioxus, Answers bind to what was drawn: 'A
handler for something the user answered or confirmed acts on what the render that drew the control showed, never on
state it reads when it runs.' Original potentially-highest interpretation PRESERVED: a control described as reconnecting
can dispatch destruction and lose every remaining process/file in that environment, outside the consequence stated on
that control. Endpoint checks establish a narrower authority route: sessions/sandbox.rs:316-331 first requires
manager.sealed_failed(host), which manager.rs:2657-2665 defines as an existing SealedHost::Failed record; otherwise it
returns Conflict and does not call finish_seal. finish_seal takes the per-host provisioning lock, then
finish_seal_locked calls hosts::finish_sealed_environment, which re-reads and requires session-owned plus
removal_deletes_machine befor

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g031/f2`.

### g031/f2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: 222bf6253bcd168db04bcbb1df20180c88306998; synthesis checked through tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: Hosts' Retry says 'Try the connection again' but can instead retry
destroying a sandbox and all files/processes still in it. Captured-render contract: HostRow:5008-5020 captures only id
in onclick and always draws that connection description; it carries no displayed operation discriminator. Parent
on_retry:2337-2380 reads hosts.peek(), computes deleting from the current HostPhase::Deleting { failure: Some(_) }, then
chooses retry_environment_delete or retry_host. The operation is therefore chosen from live state, not the render acted
on. This mismatch exists even without a delayed click when the current row already reports failed deletion, and a queued
reconnect Retry can change operation after its render. SPEC_impl.md GUI: Dioxus, Answers bind to what was drawn: 'A
handler for something the user answered or confirmed acts on what the render that drew the control showed, never on
state it reads when it runs.' Original potentially-highest interpretation PRESERVED: a control described as reconnecting
can dispatch destruction and lose every remaining process/file in that environment, outside the consequence stated on
that control. Endpoint checks establish a narrower authority route: sessions/sandbox.rs:316-331 first requires
manager.sealed_failed(host), which manager.rs:2657-2665 defines as an existing SealedHost::Failed record; otherwise it
returns Conflict and does not call finish_seal. finish_seal takes the per-host provisioning lock, then
finish_seal_locked calls hosts::finish_sealed_environment, which re-reads and requires session-owned plus
removal_deletes_machine before removal. SPEC.md Sandbox sessions explicitly says 'The seal survives a restart of its
Farhelm and is never lifted' and 'the helm finishes destroying any sealed sandbox it sees, whether or not it sent the
Delete: after a lost reply, a failed destruction or its own restart', and the initial confirmation covers every
process/file. Thus source establishes retry of already irrevocably authorized destruction, with no source route from
this endpoint to a fresh unsealed environment. Narrower established consequence: misleading action description and
operation selected from live state. Suggested bucket for that narrower consequence: other; original potentially-highest
consequence remains for independent checking. NO parent drop or lowering is treated as accepted, and no independent
check was performed by this reviewer. proposed_drop: false; retain the finding. possible_cover: none for the misleading
UI, quoted permanent-seal contract is only the proposed basis for narrowing the destructive-authority interpretation.
Confidence: confirmed source mismatch and guarded endpoint; source-only, no execution. Later fixes: hosts.rs is
byte-identical from 222bf625 to tip and retry_environment_delete/finish_seal paths are unchanged;
2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa has no diff in sessions/sandbox.rs. No later in-range fix found.
Could-be-highest: yes pending independent acceptance of the stated narrowing; all original destructive impact wording
retained.

Returned proposal (claims-018, event 14): **retain-verified**, suggested bucket **other**.

Retry's connection description disagrees with its deletion request even on a current failed-deletion row. The callback
also selects the operation from live state. The proposed other bucket depends on independent acceptance of the preserved
authority narrowing.

Original stronger consequence: An old reconnect Retry can dispatch destruction and end every remaining process/file,
beyond the consequence stated on that control. The reviewer proposed narrowing this out of highest because destruction
was already irrevocably authorized by the seal; that lowering is not accepted here.

Unresolved premise or limitation: No fresh unsealed-destruction route is shown. Independent judgment must accept that
permanently authorized seal completion removes the original unauthorized-loss interpretation before finalizing other.

Proposed basis: Retain the verified UI mismatch. Independently check the proposed consequence lowering against
SPEC.md:1506-1540 and the sealed_failed endpoint guard; this consultation does not authorize lowering or dropping it.

Existing parent disposition (event 14): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-verified-retry-description-live-dispatch-with-proposed-other-narrowing. Parent read
full018 and actual current SPEC sandbox Delete/seal passage1485-1555. Retry ordinary failed sealed resource already
permanently authorized destruction of every file/process, seal never lifts and helm finishes destruction after
failure/restart even without new Retry. Endpoint strictly sealed_failed. Proposed other for actual misleading connection
help and live-phase dispatch, not new unauthorized destruction; fresh gate must independently accept exact
permanent-prior-consent narrowing. Original old reconnect Retry→destruction highest interpretation preserved until gate.
Current ordinary wrong help means rare diagnostic filter not fullmatch; retain defect regardless lowered severity
verdict.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g031 record](records/review-through-00de0b6c/groups/g031.json). Record ID: `f2`.
