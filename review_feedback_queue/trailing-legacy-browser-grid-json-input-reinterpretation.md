# Stale xterm pages can reinterpret literal JSON paste as unbracketed input

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: After the grid-only update, a page left open from the previous build can treat a pasted JSON
document as an input instruction. Literal backslash escapes in the document can become real control characters sent
outside bracketed paste, allowing embedded commands to execute instead of inserting the document. Original c3
consequence preserved: ordinary stale-page output is misrendered and non-JSON keystrokes end the attachment. The current
grid parser/buffer replacement still works for current peers; it does not refute this distinct valid-JSON input route.
Source: parent session_view.rs:3839-3849 preserves an absent terminal kind as null; parent terminal.js:4658-4665 builds
xterm for unknown kind and :5721-5724 selects raw bytes when the removed stream notice is absent. Parent :2650-2675
intercepts text paste, :2232-2235 sanitizes literal ESC only through term-bytes.js:55-56, and :4628-4634 invokes
term.paste. The removed xterm paste function calls triggerDataEvent once with all prepared text; it wraps only when its
local CoreService decPrivateModes.bracketedPasteMode is true. That mode defaults false in this group's assigned
01085-016.patch. Parent terminal.js:4191-4196 routes onData to :4158-4159's one binary WebSocket send. Current helm
terminal.rs:762-773 forwards that frame unchanged; GridInput's tagged Text variant (grid.rs:1565-1566,1610-1612)
deserializes JSON and validates lengths (:1650-1655,1686-1693), without distinguishing the legacy browser.
HostAttach::send_input at service/grid_stream.rs:1064-1091 decodes and queues it; grid_input.rs:175 emits Text's decoded
dat

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g193/c3`.

### g193/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. TLDR: After the grid-only update, a page left open from the
previous build can treat a pasted JSON document as an input instruction. Literal backslash escapes in the document can
become real control characters sent outside bracketed paste, allowing embedded commands to execute instead of inserting
the document. Original c3 consequence preserved: ordinary stale-page output is misrendered and non-JSON keystrokes end
the attachment. The current grid parser/buffer replacement still works for current peers; it does not refute this
distinct valid-JSON input route. Source: parent session_view.rs:3839-3849 preserves an absent terminal kind as null;
parent terminal.js:4658-4665 builds xterm for unknown kind and :5721-5724 selects raw bytes when the removed stream
notice is absent. Parent :2650-2675 intercepts text paste, :2232-2235 sanitizes literal ESC only through
term-bytes.js:55-56, and :4628-4634 invokes term.paste. The removed xterm paste function calls triggerDataEvent once
with all prepared text; it wraps only when its local CoreService decPrivateModes.bracketedPasteMode is true. That mode
defaults false in this group's assigned 01085-016.patch. Parent terminal.js:4191-4196 routes onData to :4158-4159's one
binary WebSocket send. Current helm terminal.rs:762-773 forwards that frame unchanged; GridInput's tagged Text variant
(grid.rs:1565-1566,1610-1612) deserializes JSON and validates lengths (:1650-1655,1686-1693), without distinguishing the
legacy browser. HostAttach::send_input at service/grid_stream.rs:1064-1091 decodes and queues it; grid_input.rs:175
emits Text's decoded data.as_bytes() without paste framing, regardless of the real program's bracketed-paste mode.
Escaped carriage returns in otherwise literal pasted JSON therefore become real Enter input. Build interlocks checked:
parent skew.rs:32-35 deliberately does not block rendering; its component at 287-294 renders a PeerLine, not a modal or
input gate. Parent api.rs:2522 observes build skew without rejecting the response. Parent terminal.js:1561-1580 revokes
automatic recovery but explicitly retains manual control; applyCapabilityChange at 1632-1638 only cancels in-flight
automatic controllers, and sync:3245-3300 retains existing islands and permits attended mounts. Paste insert and
sendData have no build-skew guard. Exact-version supervisor hello gating does not gate browser binaries. Attribution:
the removed bundle's default-false paste mode plus the removed stream negotiation permits this route at f560058d; the
new decoder/byte-as-Text behavior is the unsafe interface, not a defect in retired xterm itself. Full-range log for
terminal.js/grid.rs/skew.rs/api.rs was inspected, and pinned-tip grid_input.rs:175 retains direct Text encoding.
Suggested bucket: highest/security or loss of work. Could-be-highest: true; successfully reinterpreting literal pasted
JSON as unbracketed command input is an established shipped route, rather than ordinary malformed-input rejection.
Confidence: confirmed source route conditional on a fresh unknown-kind old-page mount and a complete small valid
GridInput Text JSON paste; no execution or actual command outcome observed. proposed_drop: false for the
successful-input consequence. Preserve the original potential acceptance basis for independent judgment: SPEC_impl.md
Grid streams says "A page loaded from the previous helm and left open across the update must be reloaded; nothing keeps
it working." and "Keeping the stream notice for such pages was considered and not done: a reload is the whole remedy."
Its described effects are misdrawing and malformed-input detach; it does not specifically accept successful
reinterpretation into unbracketed command input. No parent drop or lowering accepted. possible_cover: exact supplied
g196/l1; related supplied g082/c2 and g083/c5. This is reconciliation of a known interface finding, not a new
independent discovery. Caveats: requires old page, unknown-kind fresh xterm surface, a still-open attachment and valid
small JSON paste before ordinary typing closes it; a locally enabled xterm bracketed-paste mode can prevent this exact
route. No reproduction, runtime test or build. Fix direction: preserve browser stream negotiation or gate incompatible
browser input before the grid decoder; rejecting ordinary malformed bytes alone is insufficient. The temporary c3 lead
is resolved to this finding.

Returned proposal (claims-121, event 11): **retain-verified**, suggested bucket **highest**.

The source establishes successful reinterpretation of a complete small JSON paste from a freshly mounted unknown-kind
old page. A literal JSON escape becomes control input outside paste framing. This is stronger than the accepted
stale-page rendering failure or malformed-input detach. The result is conditional source evidence, not an observed
command execution.

Original stronger consequence: Original removal concern: unavailable or uninterpreted output/input. Line 9 proposed
dropping accepted stale-page misrender/refusal, then line 10 reopened the no-highest conclusion. Line 11 establishes the
stronger security/work-loss possibility: JSON escapes become unbracketed control input, potentially executing embedded
commands. Preserve that stronger consequence and its conditional assumptions; do not replace it with ordinary detach.

Unresolved premise or limitation: No missing source premise for the conditional route: it requires a fresh unknown-kind
xterm mount on an old page, local bracketed mode still false, an open attachment and a complete small valid Text JSON
paste before malformed input detaches it. Actual shell/TUI command outcome and frequency were not observed.

Proposed basis: Retain highest on the established conditional input reinterpretation route. Any later attempt to apply
the broad stale-page acceptance, upgrade filter, duplication, withdrawal or consequence lowering requires the parent's
fresh independent drop check. Supplied mentions of g196/l1 and g082/c2 are reconciliation leads only; their records are
outside this batch and were not independently checked as aliases.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g193 record](records/review-through-00de0b6c/groups/g193.json). Record ID: `c3`.
