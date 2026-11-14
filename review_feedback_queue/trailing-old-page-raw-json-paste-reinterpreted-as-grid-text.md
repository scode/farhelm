# Incoming input-reinterpretation lead is incorporated into c1

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Incoming input-reinterpretation lead is incorporated into c1

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g053/l2`.

### g053/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 19 (candidate, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. This closes the incoming g196/l1 protocol-interface lead by
recording its source corroboration under original c1, now kind finding; it does not drop the concern. Original possible
consequence retained: a literal small JSON document pasted through an old raw-byte xterm page can be decoded as
GridInput::Text; escaped Enter/control characters become unbracketed program input with possible command execution and
loss of work. Could-be-highest: yes; suggested bucket highest, preserved for independent adjudication. Checks/results:
parent terminal.js routes sanitized text through term.paste to onData/sendData as one binary WebSocket message;
term-bytes.js removes literal ESC only, and the vendored xterm default bracketedPasteMode is false with one
triggerDataEvent for a prepared paste. The new helm forwards that message unchanged. grid.rs:1550-1702 shows
serde-tagged Text decode, JSON string unescaping and length-only Text validation; grid_input.rs:151-190 writes Text
bytes directly and reserves paste wrapping for Paste. These settle the requested exact wire-text/unescaping distinction.
Confidence: confirmed conditional wire/encoder route; global stale-page interlocks belong to g196, whose supplied latest
record checks deliberate reconnect and absent terminal build fence. No reproduction, generated payload or runtime
observation. proposed_drop: no behavioral drop; merge this supporting lead into c1 and supplied g196/l1 for
consolidation. c1 preserves the exact old-page reload-acceptance quotes and the unresolved scope of that proposed drop,
so no highest concern is erased here. possible_cover: same supplied g196/l1 and own c1, not an independent discovery.
Material caveats preserved in c1: old page, fresh unknown-kind surface, valid small one-line Text JSON, and no
already-enabled legacy paste wrapper; ordinary keystrokes still generally detach. No independent unresolved protocol
premise remains under l2.

Returned proposal (claims-034, event 19): **needs-parent**, suggested bucket **highest**.

Retain this conditional input-authority/work-loss concern. The protocol-side route is corroborated: valid literal JSON
can become decoded Text bytes with control characters, bypassing Paste encoding. The record says it was incorporated
into g053/c1 and g196/l1; those records are outside this batch, so the parent must verify consolidation rather than
silently close this record.

Original stronger consequence: A literal small JSON document pasted through an old page is decoded as GridInput::Text;
escaped Enter/control characters become unbracketed program input, with possible command execution, input-authority
violation and loss of user work. Preserve old page, fresh unknown-kind xterm surface, valid small one-line Text JSON,
and no already-enabled legacy paste wrapper caveats.

Unresolved premise or limitation: Parent must reconcile the supplied claims that global old-page interlocks permit the
route and that g053/c1/g196/l1 retain the full highest concern. Those named external records were not supplied to this
batch. No actual executed command or loss is observed.

Proposed basis: Retain highest and consolidate only after checking the destination record preserves this route and every
caveat. This is not an independent drop verdict. General old-page reload acceptance does not establish coverage of the
stronger consequence.

Existing parent disposition (event 19): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-highest-and-reconcile-raw-legacy-paste-canonical. Parent read full034 and
complete053c1 in033. Protocol corroboration matches same successful escaped-control Text interpretation, not
malformed-input failure. Global old-page interlocks separately checked032l6, original196 owns whole chain; preserve
fresh unknown-kind surface, complete small single-message valid JSON, no legacy paste wrapper, source-only/no executed
command or observed loss. Exact alias requires canonical196 preserving all these conditions and security boundary; no
general reload or upgrade filter acceptance. New interface evidence already part of earlier196 synthesis, compare actual
sent-input record before any redundant reopening.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g053 record](records/review-through-00de0b6c/groups/g053.json). Record ID: `l2`.
