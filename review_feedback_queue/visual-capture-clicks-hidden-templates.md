# Full visual capture clicks Templates while it is hidden

Reviewed commit: a03f32543bdaed2cbfea9cf33d43335611e071ff

## TLDR

The ordinary unscoped visual capture cannot finish after entering host management: its next Templates click targets a
hidden button.

## Details

Suggested bucket: **other**, for maintainer capture-tool availability. Confidence: **confirmed source contradiction**;
no runtime capture was attempted and no product failure is claimed.

`e2e/docs-shots/visual-review.spec.ts:837` opens Hosts. Closing Add and the uninstall refusal closes only their dialogs
(`crates/farhelm-ui/src/hosts.rs:2688–2692,3062–3075`); neither returns to Sessions. At capture line `860`, the script
clicks `.templates-button`, whose session heading is hidden while Hosts remains open (`src/list/view.rs:4131–4168`;
`assets/app.css:713`). Playwright therefore waits on a hidden button. Later selection at line `918` has the same missing
visibility premise, rather than a distinct defect. This remains at pinned tip
`9b19f8d754bf7658761ef246cfda0ea8397f4f21`.

Return to Sessions before Templates and subsequent session-row operations. No missing source premise remains for the
hidden-control contradiction; runtime occurrence was not independently measured.

Possible cover: none. The existing visual-delivery record concerns omitted report evidence, and its operational-report
discard does not describe this executable capture failure. Pending Steel capture extensions do not state a repair for
this transition. Planned is empty; no matching specification acceptance, BUGS, queue, ledger or filter coverage was
found. The rare-failure filter does not cover this ordinary path. Proposed drop: none.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g005/g005-f1`.
