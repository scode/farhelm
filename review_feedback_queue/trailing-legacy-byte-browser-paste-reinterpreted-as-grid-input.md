# Old-page stream fallback can reinterpret a literal paste as grid input

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Old-page stream fallback can reinterpret a literal paste as grid input

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g053/c1`.

### g053/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 18 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. Original consequence preserved: a page left open across the helm upgrade can draw grid messages as raw terminal output and lose its attachment on typing until reloaded. Revision from supplied g196/l1 and direct protocol source checks: some raw text is valid GridInput JSON, so malformed-input detachment is not the only input outcome. A literal small one-line JSON document shaped as Text can be decoded and its data string written to the program; JSON escapes become real control bytes, including Enter, without paste bracketing. This can turn a literal-document paste into command input and could lose user work. Suggested bucket: highest/security or loss of work for this wider input-authority consequence; the earlier no-established-highest assessment was limited to misdrawing and ordinary malformed input and must not suppress this route. Could-be-highest: yes. Source: parent terminal.js:2220-2235 sanitizes literal ESC only; :2650-2675 dispatches text pastes through insert; :4628-4634 calls term.paste; :4651-4666 chooses xterm for unknown kind. Parent term-bytes.js:55-56 does not unescape backslash sequences. The parent vendored xterm.js paste helper prepares text, brackets it only when decPrivateModes.bracketedPasteMode is true, and calls triggerDataEvent once; its default mode object sets bracketedPasteMode false. Parent terminal.js:4157-4159 encodes each sendData call into one binary WebSocket message, with :4191-4196 forwarding onData. New helm terminal.rs:762-773 forwards each binary message unchanged. At the reviewed revision GridInput is serde(tag=t) (grid.rs:1565-1566); Text carries a String (:1610-1612), decode uses serde_json::from_slice (:1650-1655), check_object merely checks the initial object delimiter (:558-565), and Text validation only bounds byte length (:1686-1693), with no ban on decoded control characters. HostAttach::send_input decodes before InputEncoder (grid_stream.rs:1064-1076); InputEncoder::encode Text is data.as_bytes(), distinct from the Paste branch and independent of the real program's bracketed-paste mode (grid_input.rs:175-176). Thus JSON escape interpretation and bypass of paste wrapping are confirmed source properties, not speculation that all old raw input must fail decode. Confidence: confirmed wire/encoder transformation conditional on the shipped legacy surface sending that complete paste; full end-to-end stale-page interlocks remain the interface owner g196's concern, whose latest supplied record establishes deliberate reconnect availability and no terminal build fence. No runtime behavior observed and no payload generated. Exact proposed_drop basis preserved for independent judgment: SPEC_impl.md, Grid streams, says ‘A page loaded from the previous helm and left open across the update must be reloaded; nothing keeps it working.’ and ‘Keeping the stream notice for such pages was considered and not done: a reload is the whole remedy.’ It describes grid misdrawing and malformed-input detachment, not specifically successful reinterpretation of a literal document. Do not accept or reject its scope here; no drop or lowering accepted. possible_cover: supplied g196/l1 is the originating same input-reinterpretation concern, with g082/c2 and g083/c5 corroboration; retain for consolidation, not a new independent discovery. Caveats: requires old page plus fresh unknown-kind xterm surface and valid small JSON paste; ordinary keystrokes generally detach. If that surface has already enabled bracketed paste from received bytes, the added wrapper prevents this exact complete-JSON route. A retained host/grid surface instead raises a stream error as recorded earlier. This acceptance cannot justify incompatible terminal-host wire behavior, which c2 separately refutes. This revises the original c1 rather than erasing its display/detach consequences or its proposed-drop basis.

Returned proposal (claims-033, event 18): **retain-verified**, suggested bucket **highest**.

The source establishes a successful input transformation beyond the originally considered misdraw/detach: a complete small JSON-shaped literal paste can decode as Text and turn escaped controls into program input without paste wrapping. The spec explicitly accepts stale-page misdrawing and malformed-input detachment, but does not clearly accept this successful reinterpretation. Ambiguous coverage means retain.

Original stronger consequence: Original high consequence: stale page misdraws grid output and detaches on ordinary malformed input until reload. Latest revision strengthens to possible command/control input and user-work loss after literal JSON reinterpretation. The earlier no-established-highest assessment is expressly superseded, not evidence to drop the stronger consequence.

Unresolved premise or limitation: Requires a stale page, fresh unknown-kind xterm surface, a complete valid small JSON paste and no active xterm paste wrapper. The JSON/control transformation is directly established. Caller-supplied g196/l1 owns the complete stale-page interface synthesis; no browser observation is claimed. If xterm already has bracketed paste enabled, its wrapper defeats this exact complete-JSON route.

Proposed basis: Retain highest with source-only trigger conditions. Parent should consolidate with caller-supplied g196/l1 (and its g082/c2/g083/c5 corroboration), not create an extra independent report. Any proposal to cover or lower the input-authority consequence needs a fresh independent check of the exact acceptance scope.

Existing parent disposition (event 18): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-highest-raw-legacy-paste-grid-input-reinterpretation. Parent read full033 bounded. Fresh unknown-kind xterm after old-page reconnect, no notice selects bytes, literal paste sanitizer removes literalESC but not JSON escapes, unbracketed paste emits once as binary whole message, helm forwards unchanged, new GridInput Text decoding writes decoded controls as ordinary input. Successful data-to-control transformation exceeds clearly named malformed detach/misdrawing acceptance; ambiguous cover retains. Preserve small whole valid message, no active paste wrapper, fresh surface/manual reconnect and no payload/runtime-frequency claims. Same196family with032l6/082/083 support requires canonical reconciliation; earlier high-only/no-highest conclusion superseded, not reason to lower.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g053 record](records/review-through-00de0b6c/groups/g053.json). Record ID: `c1`.

