# The HTTP defaults still reach an interactive old raw-paste surface after build skew

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The HTTP defaults still reach an interactive old raw-paste surface after build skew

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g017/l1`.

### g017/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. Follow-up interface checks resolve the readily accessible
global-build/caller premise without execution. TLDR/consequence unchanged from the preceding l1 revision: missing
terminal-kind JSON can mount a fresh old xterm surface, whose valid JSON paste may be interpreted as new grid Text
control rather than literal document content; unescaped carriage returns could submit embedded commands outside
bracketed paste. Preserve the original ordinary-input misdraw/disconnection consequence and all earlier proposed-drop
bases in the durable history. Own new reads at parent revision: api.rs:2483-2540 calls skew::note_build(&resp) at :2522
for side effects and does not reject a successful mismatched reply; skew.rs:30-35 explicitly states no blocking/refusal
to render/automatic reload, :125-130 gates unattended behavior only, :215-241 records the stamp; reconnect.rs:185-225
gates auto/heartbeat while :205-208 expressly leaves manual reconnect enabled. session_view.rs:1765-1848 passes
defaulted agent_kind and tab.kind to terminal specs without a build veto. terminal.js:4650-4666 chooses xterm when
spec.kind is not host, :4628-4634 installs the term.paste callback with a WebSocket-open liveness check, :2232-2235
inserts sanitized text without a skew check, :2649-2675 routes plain clipboard text to it, and :4158-4159/:4191-4196
sends one encoded onData chunk if the socket is open. These checks corroborate interactive admission after the assigned
HTTP omissions; no missing global build interlock is supported by those source paths. g196 owns and supplies the
remaining end-to-end control interpretation source: fresh xterm default bracket mode, complete paste emission, GridInput
Text JSON decode, Text encoder sending raw data bytes; exact refs remain in prior l1 and supplied neighbour record.
Confidence: confirmed for HTTP defaults, surface selection and lack of a build/paste veto in the inspected callers;
likely full highest route based on g196's source-established paste/decoder chain, not observed execution. Suggested
bucket: potential highest/security or work loss, for parent reconciliation with g196/l1. proposed_drop: false for the
valid-JSON successful-control route. The earlier exact SPEC_impl.md Grid streams :3021-3027 text ('A page loaded from
the previous helm and left open across the update must be reloaded; nothing keeps it working.' / 'Keeping the stream
notice for such pages was considered and not done: a reload is the whole remedy.') expressly names misdrawing and
malformed-input rejection, not successful reinterpretation; preserve that previous narrow proposed drop for independent
judgment rather than applying it to this stronger consequence. possible_cover: only the explicit reload paragraph, no
prohibited coverage searched. Could-be-highest: yes; no lowering/drop accepted. Caveats: requires an old page, fresh
unknown-kind xterm surface, live attachment, and valid small Text-shaped JSON paste; g196 remains owner of full
input-admission verification and this record is a cross-group duplicate/interface consequence. No unresolved readily
checkable HTTP caller premise remains. No reproduction, runtime test, build or live-install action performed.

Returned proposal (claims-008, event 10): **retain-verified**, suggested bucket **highest**.

An old page can mount a fresh raw xterm surface against the new grid socket and paste a complete valid Text-shaped JSON
message. The supervisor decodes its escaped control characters and sends the resulting bytes as ordinary Text, bypassing
paste framing. This establishes a conditional input-authority/security route by source; no runtime occurrence was
observed. The reload exception describes broken display and malformed-input rejection, not this successful
reinterpretation.

Original stronger consequence: The initial lead concerned old-page misdrawing and disconnecting on ordinary input after
upgrade. Lines 4, 5 and 8 proposed dropping that narrow behavior under the explicit reload exception, including the
assumption that input is rejected as malformed. Lines 9 and 10 withdrew the broader dismissal after the supplied g196/l1
route showed valid JSON can instead be decoded and submitted as control-bearing Text. Preserve both the earlier
display/detachment acceptance and the later possible command submission/user-work consequence. The route needs an old
page, fresh unknown-kind xterm mount, live socket and small valid Text-shaped JSON paste before a rejecting input or
reload.

Unresolved premise or limitation: No missing source premise remains for the narrow literal-JSON-to-control-byte route
inspected here. Runtime admission/order was not exercised. Actual damage depends on the decoded bytes and program;
successful conversion of escaped CR to unbracketed input already establishes the security boundary consequence
supporting highest.

Proposed basis: Retain the successful-control interpretation as highest. Keep the old proposed-drop history in the
independent register: any decision to apply the reload exception or an upgrade filter to this strengthened route, or to
lower it, needs an independent check. The record names g196/l1 as the interface owner; use underlying_key to reconcile
it with that owner's batch rather than assuming a second defect or dropping either owner's caveats.

Existing parent disposition (event 10): verified. Retain the strengthened original highest consequence without dropping
or lowering it. Parent primary verification already checked whole-run196/011/017/032/053/082/083/088/192/193 input
chain; fully read focused008 independently corroborates listing defaults, manual old-page unknown-kind mount, fresh
xterm paste mode off/one data event, literal JSON escapes surviving cleaning, stable socket framing and new GridInput
Text decoding escaped CR into unbracketed input. Conditional source route, not runtime observed: old loaded bundle,
fresh unknown-kind surface, small valid Text-shaped literal JSON before malformed input/reload. Existing known-kind grid
islands are outside this route. Current reload exception accepts broken display/malformed-input detachment but does not
clearly accept successful input reinterpretation; upgrade filter excludes security consequence. No current exact
queue/Planned/unlanded plan/BUGS/active-ledger cover. Earlier proposed ordinary-misdraw drops remain pending in full
independent register; this retention does not accept them. Preserve every alias and confidence caveat; no concrete
damage or frequency claim.

Unapplied parent proposal: retain-highest-canonical-legacy-json-input. Parentreadfull008.
ExplicitdefaultsforremovedHTTPkind/capabilityfields, unknownkindfreshxterm(shelltabtoo),
build-skewnonblocking/manualreconnect, stableWSsubprotocol, noticeabsenceclassifiedbytes
andwholeplainpastewhileinitialbracketedpasteoff establishproducer. ASCIInestedJSONescapespasssanitizer,
newTextdecoder/encoder convertsescapedCRtounbracketedcontrolinput;
sourceconditionalsecurityroutecompletewithoutreproduction.
Keepoldmisdraw/malformeddetachreloadacceptancePARTIALandoriginalbroaddismissalwithdrawal, nooldpageseverywhereclaim.
Retainhighestandcombine196canonical/011andotherswithallcaveats; exactsecurityexceptionmeansupgrade-rarefiltercan'tdrop.
Anyalias/drop/loweringneedsfreshgate; noindependentfinaldecisionnow.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g017 record](records/review-through-00de0b6c/groups/g017.json). Record ID: `l1`.
