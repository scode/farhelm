# Old-page GUI interlocks do not block the supplied JSON paste reinterpretation

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: an old browser page can remain interactive through the grid-only update, mount xterm for a
missing terminal kind and send literal pasted JSON as raw bytes that the new supervisor can decode as an input
instruction. Original potentially highest consequence preserved from supplied g196/l1: document string escapes may
become actual Enter/control input outside paste wrapping, running unintended commands and losing user work. Synthesis
result: duplicate/supporting interface record for g196/l1 latest supplied line10, not an independent discovery. Required
App/API interlock checks were completed and do not refute the fresh unknown-kind interactive-surface premise. Source:
old api.rs:2522 calls skew::note_build for its side effect but returns successful resp unchanged at :2549; only
authentication refusals have their separate error branch. old skew.rs:30–35 explicitly excludes blocking/render
refusal/automatic reload; BuildSkewNotice at :287–293 is an ordinary PeerLine, not a modal. old lib.rs:2042–2098 uses
skew only to show that notice and prioritize it over TokenPrompt, then retains the PreferencesGate/authenticated tree;
it does not inert or unmount SessionView. old session_view.rs:1867–1881 maps mismatch to unattended reconnect/heartbeat
policy, not an input veto. terminal.js:1593–1595 returns manual-only for revoked capability; :1632–1638 cancels
in-flight recovery attempts, not all live islands; :3284–3301 refuses unavailable or unattended mounts while explicit
opening/reclaim remain allowed; :3414–3436 deliberately permits reclaim and :3460–3465 manual reconnect. No backend

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g032/l6`.

### g032/l6

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e; old loaded UI examined at its parent. TLDR: an old browser
page can remain interactive through the grid-only update, mount xterm for a missing terminal kind and send literal
pasted JSON as raw bytes that the new supervisor can decode as an input instruction. Original potentially highest
consequence preserved from supplied g196/l1: document string escapes may become actual Enter/control input outside paste
wrapping, running unintended commands and losing user work. Synthesis result: duplicate/supporting interface record for
g196/l1 latest supplied line10, not an independent discovery. Required App/API interlock checks were completed and do
not refute the fresh unknown-kind interactive-surface premise. Source: old api.rs:2522 calls skew::note_build for its
side effect but returns successful resp unchanged at :2549; only authentication refusals have their separate error
branch. old skew.rs:30–35 explicitly excludes blocking/render refusal/automatic reload; BuildSkewNotice at :287–293 is
an ordinary PeerLine, not a modal. old lib.rs:2042–2098 uses skew only to show that notice and prioritize it over
TokenPrompt, then retains the PreferencesGate/authenticated tree; it does not inert or unmount SessionView. old
session_view.rs:1867–1881 maps mismatch to unattended reconnect/heartbeat policy, not an input veto.
terminal.js:1593–1595 returns manual-only for revoked capability; :1632–1638 cancels in-flight recovery attempts, not
all live islands; :3284–3301 refuses unavailable or unattended mounts while explicit opening/reclaim remain allowed;
:3414–3436 deliberately permits reclaim and :3460–3465 manual reconnect. No backendCapability/inputFrozen global check
exists in these parent paths. Source-established current-host ordinary path: missing agent_terminal decodes None in old
lib.rs:520–521; agent_spec_kind preserves it at session_view.rs:3839–3849 and :1809 puts it in the mount spec;
terminal.js:4650–4665 builds xterm for non-host, including null. spec.unavailable comes from session.stale, and the
terminal DOM's inert mark is only shown.stale (session_view.rs:3167,3231), not skew; modal isolation applies only when
an actual modal is present. A connected ordinary host with no modal therefore satisfies these gates. old
terminal.js:4628–4634 installs paste through term.paste; :2232–2235 sanitizes pasted text but has no skew check;
:4158–4159 sends raw encoded text whenever the socket is OPEN. Missing stream notice settles bytes at :5887–5915, while
the updated helm no longer emits that notice. g196 source-checks whole-message paste dispatch, fresh xterm unbracketed
default, unchanged browser-message relay and the new GridInput Text decoding/encoding; this record adds the assigned
App/API/global-admission checks, without constructing a payload or independently reproducing that lower-layer result.
Suggested bucket: highest/security or loss of work if retained. Could-be-highest: yes. Confidence: likely conditional
route, not runtime observed; premise is the old page opening/reclaiming a connected unknown-kind terminal and pasting a
small valid Text-shaped JSON document while the old surface remains unbracketed. Ordinary keystrokes usually detach
instead, and an xterm surface whose program enabled bracketed paste can prevent this particular route. Exact
proposed_drop basis preserved for independent review: SPEC_impl.md Grid streams :3022–3027 says "A page loaded from the
previous helm and left open across the update must be reloaded; nothing keeps it working" and "Keeping the stream notice
for such pages was considered and not done: a reload is the whole remedy". The full paragraph expressly discusses
misdrawing and malformed-input attachment failure, not successful instruction reinterpretation; no drop or lowering is
accepted here. possible_cover: supplied g196/l1 owns the same underlying defect; parent should treat l6 as
duplicate/interface support. No additional readily checkable App/API/isolation premise remains; incidence and clipboard
payload occurrence are unmeasured. No runtime test, payload construction, reproduction or live-install action. Lead l6
is resolved to this retained supporting finding before beginning the separate new search.

Returned proposal (claims-019, event 15): **retain-unverified**, suggested bucket **highest**.

The assigned UI/API interlocks do not block the old-page premise. Retain the possible successful data-to-input
reinterpretation and its work-loss/security consequence for reconciliation with its supplied owner g196/l1. This batch
did not read that owner's record and therefore does not independently authorize cross-batch deduplication.

Original stronger consequence: An old page's literal pasted JSON can become actual Enter/control input outside paste
wrapping, execute unintended commands and lose user-owned work. Preserve both the security interpretation and all
old-page/bracketed-paste caveats.

Unresolved premise or limitation: Independently comparing the supplied g196/l1 record and completing its whole-message,
unbracketed-paste chain is outside this batch's assigned interface check. Clipboard occurrence and runtime incidence are
unmeasured.

Proposed basis: Retain highest while the successful-interpretation and upgrade-filter security boundary are reconciled.
Do not treat reload-only compatibility wording as acceptance of every possible old-page consequence.

Existing parent disposition (event 15): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-highest-pending-canonical196-reconciliation. Full019
directlyconfirmsoldpage/App/APIpremises: skewnoticeleavesauthPreferencesGatetree, successfulresponsesremain,
missingkindmountsxterm, rawwholeTextJSONparseanddecodedbytesallowed.
Successfulinstructionreinterpretationdistinctfromexplicitlyacceptedmalformeddetach/misdrawing.
Preservebracketedpastecaveat/unbracketedsurfacewholemessagechainfrom196, no payload/runtime/frequencyclaim.
Alreadyretainedsame196rawJSONfamilyand017supportingfindingmustreconcilefullcanonicalbeforeexactalias; no
broadreloadcoverorupgradefilterwhilepossibletrustboundaryunsettled. Sourceinterfacesalreadyreviewedsynthesizedearlier;
compareactualhistoricaldeliverybeforereopeningagain.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g032 record](records/review-through-00de0b6c/groups/g032.json). Record ID: `l6`.
