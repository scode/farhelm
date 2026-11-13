# An old open page requires reload after the grid-only update

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: An old open page requires reload after the grid-only update

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g196/c3`.

### g196/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (candidate, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. TLDR and original consequence preserved: an old open page can reconnect with xterm, misdraw grid output and have ordinary typing end the attachment until reload. Turn 2 source synthesis establishes the condition more precisely: at the parent revision, lib.rs:514-521 defaults a missing agent_terminal to None, session_view.rs:3839-3849 passes that unknown kind through, and terminal.js:4651-4666 chooses xterm for an unknown kind. Its :5721-5723 and :5891-5930 interpret absent stream notices as bytes. A mount retaining a known host kind instead keeps the grid and refuses the missing grid notice; therefore not every old-page reconnect misdraws. The old xterm onData route (:4157-4158 and :4191-4196) sends raw encoded bytes, whereas this revision's grid_stream.rs:1064-1076 decodes a GridInput before encoding or queuing input. connection.rs:511-550 removes the attachment and requests forwarder shutdown on a decode/send failure. This establishes ordinary malformed input ending the attachment, not stopping its program. The old page's reconnectControls (:1561-1580) disables automatic attempts after the policy withdraws capability while retaining manual reconnect; reconnect.rs:209-217 supplies that capability gate. Exact accepted contract from SPEC_impl.md, 'Grid streams': 'A page loaded from the previous helm and left open across the update must be reloaded; nothing keeps it working.' The same paragraph states: 'Keeping the stream notice for such pages was considered and not done: a reload is the whole remedy.' Its full exact paragraph remains preserved in the prior c3 revision. Suggested bucket remains high/material UX if treated as a defect; there is no lowering of the original consequence. Confidence: confirmed source route and specification acceptance; no execution observed. proposed_drop: explicit specification acceptance quoted here and in the prior revision; still only proposed, no drop accepted. possible_cover: none; forbidden coverage sources were not consulted. Could-be-highest: no established security or loss-of-work route. This conclusion is limited to the established misdrawing/ordinary-key rejection route, and does not claim arbitrary raw payloads could never decode as valid grid input. Caveats: the time taken for the page to observe build mismatch was not measured. One attempted context path, src/terminal.rs, did not exist; the actual caller was then checked in session_view.rs. No reproduction, runtime test or build performed. Superseding synthesis caveat: l1 now establishes a valid-JSON paste route beyond ordinary malformed-input detachment. The prior statement that no security/loss-of-work route was established is no longer the overall conclusion. Preserve c3 as the separately proposed drop of the documented display/attachment consequence, but do not use it to drop l1. l1 is highest, likely on its named old-page/fresh-xterm conditions, with exact specification acceptance presented for independent judgment. No drop has been accepted.

Returned proposal (claims-122, event 11): **covered**, suggested bucket **high**.

The exact display failure and ordinary malformed-input detachment are explicitly accepted. The successful JSON reinterpretation in g196/l1 is a separate, stronger consequence and must remain retained; this proposal cannot dispose of it.

Original stronger consequence: Original c3: old open page reconnects, misdraws grid output and ordinary typing ends the attachment until reload. Final event preserves that consequence but withdraws its earlier overall assurance that no security/loss route exists, citing g196/l1's highest-priority paste route.

Unresolved premise or limitation: No missing premise for the narrow accepted consequence. Timing/incidence was not measured. Successful interpretation of valid pasted JSON is not established as accepted by this paragraph.

Proposed basis: Exact specification coverage only for misdrawing and malformed-input attachment failure. Keep the independently recorded successful-command reinterpretation consequence outside this proposed coverage decision.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g196 record](records/review-through-00de0b6c/groups/g196.json). Record ID: `c3`.

