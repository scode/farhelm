# Removed stream-kind negotiation matches protocol 77

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Considered whether deleting stream notices and the adoptGrid compatibility path breaks an
attachment that still needs the retired byte renderer. The pinned contract explicitly removes this compatibility: all
new attachments carry grid data; previous loaded pages must reload. The new terminal is created before connecting
(3387–3393), and binary data always enters receiveGrid (4152–4165). This is the intended change, not an omitted
supported renderer. Contract: SPEC_impl.md, Grid streams, lines 3014–3027: "Protocol 77 removed the byte streams with
the old terminals and xterm.js: every attachment's data channel carries a grid stream, so Attach, Attached and the
browser's socket say nothing about it" (inline backticks omitted). Exact further text: "A page loaded from the previous
helm and left open across the update must be reloaded; nothing keeps it working." It explicitly describes old-page grid
misdrawing and malformed input ending the attachment, and says "a reload is the whole remedy."

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g083/c5`.

### g083/c5

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 17 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. TLDR: Considered whether deleting stream notices and the
adoptGrid compatibility path breaks an attachment that still needs the retired byte renderer. The pinned contract
explicitly removes this compatibility: all new attachments carry grid data; previous loaded pages must reload. The new
terminal is created before connecting (3387–3393), and binary data always enters receiveGrid (4152–4165). This is the
intended change, not an omitted supported renderer. Contract: SPEC_impl.md, Grid streams, lines 3014–3027: "Protocol 77
removed the byte streams with the old terminals and xterm.js: every attachment's data channel carries a grid stream, so
Attach, Attached and the browser's socket say nothing about it" (inline backticks omitted). Exact further text: "A page
loaded from the previous helm and left open across the update must be reloaded; nothing keeps it working." It explicitly
describes old-page grid misdrawing and malformed input ending the attachment, and says "a reload is the whole remedy."
Suggested bucket: no defect; supported-user consequence considered was high/material UX. Could-be-highest: no
established process/work-loss/security route from the removed UI negotiation; attachment loss and misdrawing are the
identified effects. This candidate does not endorse or review server-side termination of old processes. Confidence:
confirmed source and explicit specification acceptance; no execution. proposed_drop: explicitly accepted by
SPEC_impl.md, Grid streams, cited text at f560058da19e02716e6f943883a9508b447aff4e. possible_cover: none beyond
specification; no excluded coverage sources searched. Caveat: UI/backend build and protocol compatibility enforcement
are interfaces, not reviewed wholesale here. Turn-2 synthesis: caller-supplied g079 legacy-container attachment guard
and g081 sanitizer/read-refusal analysis corroborate the retired/surviving path distinction. They do not certify every
surviving component. The compatibility scope here remains exactly SPEC_impl.md Grid streams: "Protocol 77 removed the
byte streams with the old terminals and xterm.js: every attachment’s data channel carries a grid stream" (typographic
apostrophe normalized in this revision; original citation retained above), and the original explicit reload acceptance.
l1 is a surviving-grid lifecycle/test defect and is not covered by renderer retirement. No new supported byte-stream
caller was established. Proposed drop remains pending caller determination; no drop accepted. Discovery-turn interface
synthesis (not a new independent discovery): latest g196/l1 supplies an established old-page route beyond the
misdrawing/malformed-input effect considered above: a fresh legacy surface can send one pasted JSON document as raw
bytes; the new endpoint can accept it as GridInput Text and turn escaped CR into unbracketed program input. Preserve all
prior c5 wording and bases, but supersede its broad could-be-highest:no statement for this wider compatibility
consequence. Potential highest/security or loss of work must be retained; confidence likely with old-page unknown-kind
mount and valid JSON paste premises named in g196/l1. The reload-acceptance quote does not explicitly accept successful
reinterpretation of pasted JSON; proposed_drop is not accepted and must not be generalized from display breakage to this
route. possible_cover: caller-supplied g196/l1 is the related existing finding for consolidation, not searched coverage.
No runtime evidence. Local discovery will not claim this already-known interface as a new finding; source boundary
checks follow. Discovery source-boundary checks for the already-known g196/l1 consequence: directly read parent
terminal.js4628–4634 (attachment insertion calls term.paste),4650–4666 (unknown kind mounts xterm),5718–5723 (missing
stream notice chooses bytes), child helm terminal.rs762–773 (binary messages forwarded to send_grid_input), and child
grid_input.rs175–176 (Text is raw data bytes, separate from Paste). These corroborate the supplied old-page route and
the distinction from ordinary malformed-input failure. No new route or independent new finding is claimed. Potential
highest classification and unresolved proposed compatibility drop remain as revised; no drop or lowering accepted.

Returned proposal (claims-054, event 17): **retain-verified**, suggested bucket **highest**.

The latest record supersedes the original broad no-security conclusion. The narrow display/malformed-input compatibility
break is explicitly accepted, but the same transport can successfully decode a pasted JSON document as Text and deliver
its escaped carriage return as unbracketed program input. Retain that conditional security mechanism. This is source
evidence, not an observed command execution.

Original stronger consequence: Lines 5 and 13 proposed dropping display breakage/attachment rejection as explicitly
accepted and said no highest route was established. Lines 14 and 17 explicitly supersede that broad statement with
potential command execution, security or user-work loss from pasted JSON interpreted as Text. No drop or lowering was
accepted; server-side termination of old processes remains outside this claim.

Unresolved premise or limitation: No runtime observation of the complete old-page sequence. The source mechanism
requires an admitted fresh legacy/unknown-kind surface, an open socket, and one complete valid GridInput JSON object
delivered in a data event. Actual user harm additionally requires meaningful program input in its data; arbitrary
ordinary paste usually fails JSON decoding. These narrow trigger conditions do not erase the source-established
escaped-CR route.

Proposed basis: Retain the conditional JSON reinterpretation route at highest; separate its security consequence from
the narrowly accepted reload/misdrawing behavior. g196/l1 is a supplied related label for the parent to reconcile, not
an independently checked alias in this batch. Any dismissal or lowering based on renderer retirement needs the
independent check.

Existing parent disposition (event 17): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-highest-successful-old-page-JSON-Text-reinterpretation. Full054 old fresh unknown-kind
xterm/no stream notice, default unbracketed paste one data event→binaryforward→taggedTextJSON controls decoded→ordinary
OneOff input establishes conditional successful reinterpretation, not accepted malformed detach. Valid complete
object/open admitted socket/fresh surface/no legacy wrapper and actual meaningful input conditions preserved; no runtime
execution/frequency claim. Original display/reload-only no-highest assessment explicitly superseded.
Reconcile196/082/053/032 canonical, not a new independent item or broad retirement/filter acceptance.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g083 record](records/review-through-00de0b6c/groups/g083.json). Record ID: `c5`.
