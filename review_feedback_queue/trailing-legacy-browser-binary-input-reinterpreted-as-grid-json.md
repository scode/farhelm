# An old page can turn a literal JSON paste into unbracketed terminal commands

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: An old page can turn a literal JSON paste into unbracketed terminal commands

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g011/c1`.

### g011/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

# An old page can turn a literal JSON paste into terminal commands

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e

## TLDR

A page left open through the grid-only update can interpret a pasted JSON document as a terminal-input instruction.
Instead of inserting that document literally, it sends the document's data field to the program; JSON escapes for
carriage return become real unbracketed Enter input. In a shell tab this can run commands the user intended only to
paste as document text.

## Details

Discovery-turn revision of original c1 from lead to finding, preserving its original old-page misdrawing, missing-notice
refusal and ordinary malformed-input detachment consequences and prior proposed-drop basis. The new successful-input
route supersedes c1's earlier broad no-highest conclusion. No parent drop or lowering has been accepted.

Source validated directly this turn: the preceding page's terminal.js:2650-2675 dispatches text clipboard content
through insert; :2232-2235 sanitizes only through term-bytes.js:55-56 (removes literal ESC/end markers, not JSON
backslash escapes). Its installed callback at :4628-4634 calls term.paste. The preceding vendor/xterm.js:1 paste routine
converts literal CRLF/LF, adds markers only when its own decPrivateModes.bracketedPasteMode is true, and calls
triggerDataEvent once with the whole prepared text. Its default modes set bracketedPasteMode:false. Parent
terminal.js:4191-4196 sends that onData result through sendData at :4157-4158 as one UTF-8 WebSocket binary message. The
reviewed helm's terminal.rs:762-773 forwards that message unchanged as one grid input frame.
farhelm-proto/src/grid.rs:1565-1566 uses serde's internally tagged t field, :1610-1612 defines Text { data }, :1650-1655
parses JSON, and :1686-1693 validates Text length without rejecting decoded control characters. grid_input.rs:175
encodes Text.data as its raw bytes, outside paste wrapping regardless of the real program's bracketed-paste mode.
service/grid_stream.rs:1064-1090 decodes, encodes and queues that accepted message. Thus a small one-line pasted JSON
Text document can be reinterpreted into actual control-bearing program input. The earlier malformed-input check does not
refute a valid input document.

Caller prerequisites: earlier direct checks established unknown-kind mounting and missing-notice byte interpretation at
parent terminal.js:4651-4667,5719-5725,5886-5924. Parent-supplied latest g088/l2 separately confirms missing
agent_terminal/tab.kind defaults, a live ordinary session's terminal admission, fresh mount despite build skew, and no
modal/input veto from BuildSkewNotice. Parent-supplied g196/l1 and g082/c2 identify this same full paste route, not an
independent new discovery. My prior direct reconnect checks confirm automatic retries stop once mismatch is observed but
manual attachment remains; that gate does not reject raw binary paste on an already open socket. New hello protocol 77
excludes older supervisors, not the old browser page.

Contract: SPEC_impl.md:3021-3027 explicitly describes old pages misdrawing and says "what is typed into it ends the
attachment as malformed input." Successful interpretation of document contents as control input is a distinct
consequence. The product's deliberate user-paste delivery does not make JSON decoding part of the user's intended
literal paste. Suggested fix direction: preserve compatible page negotiation or reject incompatible browser input before
routing, so raw legacy bytes cannot masquerade as grid messages. No implementation change is requested or made by this
review.

Suggested bucket: highest/security or loss of work, for an established conditional route from literal paste to
unbracketed commands; no particular destructive command or loss was observed. Could-be-highest: yes. Confidence: likely,
naming runtime trigger conditions: a fresh unknown-kind legacy xterm surface remains attached until a small
valid-Text-JSON paste, with no misparsed output having enabled that surface's initially-off bracketed-paste mode. The
source shows the necessary code paths and defaults; occurrence and frequency were not executed. No readily checkable
source premise remains missing for this conditional route.

proposed_drop: false for successful input reinterpretation. Preserve the exact prior proposed drop for ordinary
display/malformed-input failure: "A page loaded from the previous helm and left open across the update must be reloaded;
nothing keeps it working" and "Keeping the stream notice for such pages was considered and not done: a reload is the
whole remedy." Whether that general reload acceptance also accepts successful command reinterpretation requires
independent judgment; it cannot be treated as a source refutation. possible_cover: parent-supplied g196/l1, g082/c2,
g083/c5 and g088/l2 for consolidation; no coverage-source search. Caveats: known-host direct-grid old mounts instead
refuse missing notice; ordinary keystrokes generally fail decoding; an old renderer with bracketed paste already enabled
would not send this particular valid-JSON raw frame. All evidence is source and contract analysis. No runtime test,
reproduction, build or live-install action was performed.

Whole-range attribution and persistence checks: the exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a already has
mandatory first stream notices (helm terminal.rs:790-806) and the page asks grid=1 then adopts the grid on its grid
notice (terminal.js:4725-4733,5654-5656,5944-5951). The f560058 diff removes stream_notice_first and the notice-first
branch, while changing inbound binary handling to unconditional grid input. Thus the old-page raw/structured ambiguity
is attributable to this in-range removal, rather than merely predating an assigned commit. The source history from
f560058 through inclusive 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db for the relevant transport, page, encoder and grid
protocol paths contains only the later comment closeout commits 08399d092d20815ced6619c26df2391944ae2b4a and
4b489b4f2eeade14f119b186ecb16eeaaf027021. These history checks are source attribution, not runtime evidence. An
initially broad context diff query was clipped; its omitted output was not claimed as reviewed, and the relevant
notice-first production source and compact deletion matches were read separately.

Persistence follow-up: read the complete compact diff for those five production paths from f560058 to the inclusive
pinned tip. It contains documentation/comment edits in grid.rs, grid_input.rs and service/grid_stream.rs only, with no
behavioral repair to the established route; terminal.js and helm terminal.rs are unchanged in that subrange.

Returned proposal (claims-005, event 11): **retain-verified**, suggested bucket **highest**.

Source establishes a conditional route from a literal JSON paste in a fresh legacy xterm surface to decoded unbracketed
control input. Ordinary malformed-input detach and display breakage are accepted, but the specification does not
explicitly accept successful command reinterpretation. Preserve the stronger consequence and the prior proposed drop
separately.

Original stronger consequence: Lines 1,3,5 proposed dropping ordinary old-page misdraw/refusal/malformed detach as
accepted and stated no highest route. Line 6 introduced the valid-message concern; lines 7,9,11 supersede that broad
no-highest conclusion with literal JSON becoming real unbracketed Enter/control input capable of running unintended
shell commands. No particular data loss or attack was observed. No parent drop was accepted.

Unresolved premise or limitation: Occurrence and frequency were not executed. The established route is conditional on a
fresh unknown-kind legacy xterm surface remaining attached until a small valid Text JSON paste, while its initially-off
bracketed-paste mode has not been enabled by misinterpreted output. Known-kind direct-grid mounts instead refuse the
missing notice; ordinary typing generally fails JSON decoding. The old client listings' absent optional kinds and mount
gates support the conditional path; no claim that every stale mount is affected.

Proposed basis: Retain the successful-input reinterpretation as highest from its conditional command-execution route. If
the parent proposes to apply the general reload acceptance, an upgrade filter, a duplicate disposition, or any
consequence lowering, send that exact proposal and the preserved history to an independent drop check. Aliases mentioned
in the supplied history belong to other batches and were not independently compared here.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-highest-canonical-legacy-json-input. Parentreadfull005.
Sourceestablishesoldloadedpagefreshunknownkindxtermcanmount/attachthroughmanualskewdespitemissingnotice,
offbracketedpasteonebinaryevent literalvalidTextJSON acceptednewgriddecoder, decodedCRqueuedasTextoutsiderealpaste.
Thisis successfulreinterpretation/controlinput, notacceptedmisdraw/malformedinputdetachorreload-onlydisplaybreakage.
Retainhighestconditionalunintendedcommandroute, noexecutedpayload/harm/frequency/allstalesurfacesclaim.
Knownkindgridmissnoticefailsseparate. Reconcile017l1/196l1andothersaliases preservingallsource/producerpremises;
earlygenericreloadacceptanceonlypartialdisplay/refusalscope. Securityboundaryexcludesupgrade-rare filter.
Freshcheckanyproposeddrop/lowering, don'ttreatgeneraloldpageunsupportedasacceptinginputtransform.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g011 record](records/review-through-00de0b6c/groups/g011.json). Record ID: `c1`.
