# Old raw-byte pages can reinterpret a literal JSON paste as grid text input

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: An old page left open through the grid-only update can send a literal JSON paste as terminal instructions: its data field, including escaped carriage returns, becomes actual unbracketed input instead of inserting the literal document. Original old-page misrendering and ordinary malformed-input detachment consequences remain preserved.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g082/c2`.

### g082/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 22 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. TLDR: An old page left open through the grid-only update can send a literal JSON paste as terminal instructions: its data field, including escaped carriage returns, becomes actual unbracketed input instead of inserting the literal document. Original old-page misrendering and ordinary malformed-input detachment consequences remain preserved. Details: this turn directly checked the unknown-kind caller premise: parent UI lib.rs:514–521 defaults absent agent_terminal to None; session_view.rs:3839–3852 preserves that absence outside the pending-restart case, and :1806–1810 passes it into spec.kind. Parent terminal.js:4657–4666 then selects xterm. Its text paste goes through onPaste/insert (:2650–2675,:2232–2235), installed term.paste (:4628–4634), and one encoded WebSocket message from onData/sendData (:4157–4158,:4191–4196). Parent term-bytes.js:55–56 removes literal ESC markers only, preserving JSON backslash escapes. Parent vendor/xterm.js:1 paste routine converts literal line endings, adds markers only if decPrivateModes.bracketedPasteMode is true, and calls triggerDataEvent once; that mode initially is false. Reviewed terminal.rs:762–773 relays binary input unchanged; grid.rs:1564–1566,1610–1612,1650–1655,1686–1693 deserialize a tagged Text message and validate its size without rejecting decoded control characters; grid_input.rs:175 outputs Text.data bytes without paste wrapping. service/grid_stream.rs:1064–1090 supplies the actual decoder/encoder/queue path. Thus a small one-line literal JSON document shaped as a Text input can become program input containing real CR, outside the real host's bracketed-paste markers. Suggested bucket: highest/security or loss of work; unintended command input is an established conditional shipped route, though no destructive command execution was observed. Confidence: likely, naming runtime conditions: a fresh unknown-kind legacy surface remains attached until paste, with no misparsed output having enabled its bracketed-paste mode. These are trigger conditions rather than missing source paths; the default and complete-message dispatch were checked. proposed_drop: earlier explicit reload acceptance remains a proposal requiring independent check, not accepted for this stronger consequence. Exact pinned SPEC_impl.md, Grid streams: 'A page loaded from the previous helm and left open across the update must be reloaded; nothing keeps it working.' Further exact text: 'and any reconnect from it, automatic in that window or manual, misdraws the terminal: that page builds its old terminal component for a terminal whose kind the listing no longer names and takes the missing stream notice for an older helm's byte stream, so it draws grid messages as terminal output, and what is typed into it ends the attachment as malformed input. Keeping the stream notice for such pages was considered and not done: a reload is the whole remedy.' This names malformed-input detachment, not successful reinterpretation of valid JSON as instructions. possible_cover: supplied exact neighbour g196/l1, not an independently new defect or accepted disposition; no prohibited coverage sources searched. Could-be-highest: true. The original c2 no-highest conclusion is superseded while its exact acceptance basis and caveats are retained. Missing premises after investigation: none for the conditional source route; no runtime incidence/frequency or destructive result was measured. Boundaries: this is removed page/helm negotiation and legacy raw-input interpretation, not a new proper-grid paste defect or terminal-host protocol change. No reproduction, runtime tests, build or live-install action.

Returned proposal (claims-053, event 22): **retain-verified**, suggested bucket **highest**.

Source establishes a conditional route from a literal pasted JSON document to unbracketed program input containing decoded control characters. The accepted old-page misrender/malformed-input failure does not explicitly accept successfully interpreting that document as instructions. Retain the stronger consequence and contract ambiguity.

Original stronger consequence: Initial candidate proposed dropping old-page misrendering/detachment as explicitly accepted, with could-be-highest=false. Events 16/19/22 reopen that conclusion: literal JSON can become unbracketed command input, potentially executing a command or losing work. Preserve both the original reload/malformed-input acceptance and the newer highest consequence; no drop was accepted.

Unresolved premise or limitation: None for the conditional parser/paste route. Trigger requires a newly constructed unknown-kind legacy surface that remains attached until paste, with bracketed-paste mode still false (no misparsed output enabled it). No observed destructive command or runtime incidence is claimed.

Proposed basis: Retain highest on an established conditional unintended-input route. Any decision that broad old-page acceptance covers it, any filter dismissal, deduplication or consequence lowering requires the independent drop check. This consultation does not make that decision.

Existing parent disposition (event 22): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-highest-successful-legacy-paste-Text-reinterpretation. Parent read full053. Fresh unknown-kind xterm, no enabled legacy paste wrapper, literal small complete JSON paste preserved through sanitizer/onebinarymessage, new tagged Text parser decodes controls and queues ordinary bytes. This is successful unintended input, beyond named accepted malformed detach/misdrawing; reload does not undo delivered input. Source conditions remain explicit, no runtime command execution/frequency/loss observed. Preserve original reload-only low assessment and later reopened highest route, reconcile196/053/032 aliases without dropping caveats. Broad old-page or upgrade filter coverage is unclear and therefore not accepted.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g082 record](records/review-through-00de0b6c/groups/g082.json). Record ID: `c2`.

