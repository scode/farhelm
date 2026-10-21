# Guarded alternative can duplicate an uncertain launch

Reviewed commit: f1d71445d629f8ee244a592b2173507e32b8c343

## TLDR

Choosing “start guarded session instead” after an uncertain YOLO launch can start a second session while the earlier,
authorized process continues. Whether this violates the intended meaning of “instead” remains a contract question.

## Details

Suggested bucket: **high, conditional on presenting one intended launch**. Confidence: **confirmed conditional source
sequence; acceptance and presentation interpretation unresolved**. No new permission bypass or loss of user work is
established, and trigger frequency was not measured.

An ordinary-folder YOLO create can succeed while its response is lost. If host policy changes to require confirmation,
an unchanged retry is refused before supervisor reconciliation
(`crates/farhelm-helm/src/sessions.rs:1608–1630, 1668–1685`; `yolo_guard.rs:59–76`). The refusal retains the unresolved
key (`crates/farhelm-ui/src/list/create_form.rs:6031–6038`). Guarded changes permissions and clears that key
(`5205–5213,6474–6496`), then allocates a new one (`5711–5729`). The first process remains authorized and running. The
question says nothing has started and offers Guarded “instead” (`yolo_confirm.rs:278–296`).

The ordinary-folder regression describes a new choice and requires a different key, but establishes zero existing
sessions before the answer (`e2e/tests/yolo-guard.spec.ts:160–164,217,228–230`). The checkout regression mocks an
initial refusal. Neither covers an uncertain successful first launch.

Possible cover: `TRIAGE_OUTCOMES.md:10412–10413,17292–17301` accepts new keys for real permission/template changes;
`SPEC.md:866–868` protects unchanged retries. Those rules support key allocation for deliberate edits, but do not
clearly settle the same-launch Guarded alternative described at `854–856,886–888`. Independent auditor `g132` returned
**ambiguous**. Proposed drop: none for this residual. Resolve that intent/presentation contract before prescribing
retry-identity changes.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, reopened `g048/c3`, independently checked by
`g132`. The unsupported permission-bypass framing was withdrawn and is not this finding.
