# Replace selection and moved-on focus rules disagree

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain other as a confirmed contract conflict; the
practical defect remains unverified. `list/view.rs:108–119,5950–5955` keeps source-selected Replace despite input.
`SPEC.md:1376–1377,1905–1907,2964–2965` expressly permits that selection, while `3225–3226,3244` makes the moved-on rule
override older focus wording. `SPEC_impl.md:1192–1194` keeps the Replace exception. No exact later resolution or current
recording cover settles this precedence or the genuine-removal successor rule.

## TLDR

The specifications give conflicting answers about opening a replacement after the user types while its source remains
selected. Current code follows the explicit Replace selection rule. Whether that violates the new focus policy remains
unverified after investigation and needs a product-contract clarification.

## Details

Bucket: **other** for the established specification conflict. Confidence: **confirmed source behavior and contract
conflict; unverified after investigation for a practical correctness defect**. Preserve the original conditional high
navigation-impact suggestion if the broad moved-on rule is intended to supersede Replace. No security or work-loss route
is established.

Reviewed source change: `112fd9d812e95df7f69f66c85820d76faaf89582`; the competing focus rule is added at the reviewed
tip. `crates/farhelm-ui/src/list/create_form.rs:7048-7054` skips activity completion checks for Replace with.
`list/view.rs:110-116,5620-5624` requires the submitted owner and selected source, then 5674 selects the replacement.
Plain Replace also checks the selected source at 3023-3032 rather than intervening activity.

The affirmative contracts are specific: `SPEC.md:1350-1351` names both Replace and Replace with; 2919-2921 says Replace
selects its replacement while its source is still selected. `SPEC_impl.md:1154,1184-1185` expressly preserves this
exception. The countercontract is also explicit: “Keyboard focus”, 3198-3200 claims precedence over older focus wording,
and 3217 says a moved-on result switches no session.

The reviewer read both source routes and the full relevant replacement, removal and focus rules. Genuine removal still
requires successor choice (`SPEC.md:2910-2921`), and the focus section accepts terminal focus after another actor
deletes the selected session. The source holds selected-source absence while replacement progress exists
(`view.rs:2022-2029`), then selects its replacement directly; ordinary disappearance later uses automatic history
selection at 3745-3817. Merely adding a press check could let that fallback switch to another history entry after the
source is deleted, rather than preserve what the user expected.

Accessible source and conflicting specifications cannot settle intended precedence or successor behavior. A maintainer
clarification giving one acceptance target would. Do not call current code a confirmed implementation violation, or
change it in isolation before reconciling replacement with genuine-removal selection. g016 initially treated the
Replace-specific text as acceptance, then revised that exclusion after cross-group synthesis.

Possible cover is the exact affirmative Replace text above. Under recording rules ambiguous coverage does not suppress
the conflicting remainder. No current queue/ledger item, indexed unlanded plan, Planned item, accepted bug or filter
settles it. No runtime check could choose the intended contract, and none was run.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `replace-selection-contract-conflict`; `g022/f2`,
`g016/l2`. Original claims, proposed exclusion, later revision and investigation remain in
`prep/groups/<group>/findings.jsonl`.
