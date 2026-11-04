# Discord feedback link omits the external-link indicator

Reviewed commit: addba1f54182a1462da2af0791bcc51c29b12e2e

## TLDR

The feedback dialog's Chat on Discord link omits the arrow used to identify links that open outside Farhelm. The words
and destination are correct; the missing part is the external-destination affordance.

## Details

`crates/farhelm-ui/src/feedback.rs:279–286` renders an anchor to `DISCORD_URL`, invokes the shared external-page opener
and labels it only `Chat on Discord`. `SPEC_impl.md`, “GUI: Dioxus,” 532–536 at the feature commit requires: “A link
that opens a page outside the app ends in `↗`; an inline action inside the app does not.” The same rule predates the
plan at `5e155da11b72c25bde82c5b34daff9887cf233ee:SPEC_impl.md:526–530`.

The plan at that earlier commit, `plans/queue/discord-link-in-app.md:81–85`, acknowledges the established arrow but
makes appending it an executor choice to log as a decision. Its own authoritative-spec statement does not authorize that
exception. The recorded maintainer wording establishes the words `Chat on Discord`, without an explicit waiver of the
separate suffix. `SPEC.md`, “Sending feedback,” 3376–3377 specifies the words/opening behavior and supplies no such
exception.

`crates/farhelm-ui/assets/app.css:2419–2438` only styles the link and supplies no arrow pseudo-element. The existing
external welcome link at `welcome.rs:62–66` explicitly includes the suffix. The anchor remains unchanged at
`c5ae8548439cc5bfa33c11ef7ce8da6129d8072a:feedback.rs:279–286`, and the rule remains at tip `SPEC_impl.md:572–576`.

Retain the words and append the external-link indicator, aligning exact accessible-name/text assertions in
`e2e/tests/feedback.spec.ts:382–395` and `e2e/tests/keyboard-scroll.spec.ts:450`; alternatively, obtain an explicit
specification exception. No target-opening or security failure is alleged.

## Confidence and priority

Confidence: confirmed source/specification mismatch, with no runtime claim. Suggested bucket: `other`.
`could_be_highest: false`. `proposed_drop: null`.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established.

Grouped findings: `g028/g028-l4` and `g053/g053-l6` identify this same missing external-link indicator.

## Provenance

Review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`: `g028-l4`, latest `g028/findings.jsonl` physical line17, reviews the
plan at `5e155da11b72c25bde82c5b34daff9887cf233ee`; `g053-l6`, latest `g053/findings.jsonl` physical line7, reviews the
feature commit above. Both original owner/ID pairs are preserved for the grouped finding.
