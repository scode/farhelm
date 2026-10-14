# Long launcher refusals can hide the destructive Replace warning

Reviewed commit: d65df9a95b691a93073b222129979d7a096fa688

## TLDR

After a long refusal, Replace can remain visible while its warning about killing the source session is below the
viewport.

## Details

Suggested bucket: `highest`. Confidence: likely; source ordering, retained error and guard propagation are confirmed.
Browser geometry, the precise clipping threshold and the destructive sequence were not reproduced.. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

The new refusal block at crates/farhelm-ui/src/list/create_form.rs:4611–4620 precedes the source warning at 4636–4641.
Error text has no height bound; app.css:4620–4635 caps and scrolls the whole dialog, while 5472–5474 only adds margins.
Long diagnostics are reachable without a malicious peer: supervisor/src/service/core.rs:3380–3385 quotes an entire
invalid relative directory, including spaces that permit wrapping. Folder correction at create_form.rs:5203–5221 retains
that error until submission. Trigger: obtain such a refusal, correct the directory, and return to the top; if an
initially ended source has meanwhile restarted, its updated warning can be below the long error. Replace remains
available and sends the permissive guard derived from the source's current state (1483–1492, 2964–2967, 4371–4383),
despite the warning being outside the visible area. This can terminate newly running work. The parent placed errors
after the controls and warning. Keep the destructive warning adjacent to Replace and ahead of unbounded diagnostics;
cover a long refusal followed by a source-state change. Scope: sighted users of Replace with.

Possible cover: TRIAGE_OUTCOMES.md:5524–5551 fixes the earlier missing-warning/guard problem, but does not address
displacement by the newly inserted refusal. SPEC.md:797–810 and 824–836 require visible consequence-bound confirmation;
they do not accept this behavior. The pinned-host-dialog-chrome queue item concerns different dialogs and clipping. No
matching Planned or BUGS entry found.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: create_form.rs:5011–5020 still places
the unbounded refusal before the warning at 5036–5041; app.css:5158–5169 and 6066–6068 retain whole-dialog scrolling
without a refusal height bound. Current SPEC.md:955–1005 retains the confirmation contract. No fixing commit identified.

Review coverage: full commit diff read: true. No commit diff unread. Unchanged portions of the large launcher, backend
and specification files were inspected only where relevant to changed behavior. TRIAGE_OUTCOMES.md bodies outside the
listed ranges were screened by headings and searches rather than read in full; TODO buckets outside Planned were not
read in full. An initial broad ledger search was truncated; conclusions use subsequently opened entries and bounded
heading searches. Browser geometry and the proposed harmful sequence remain unverified.

Runtime evidence: None, as requested by default. No checkout files or scratch artifacts created, modified or deleted. No
nested agents spawned. Independent review of the proposed highest-candidate drop remains outstanding.
