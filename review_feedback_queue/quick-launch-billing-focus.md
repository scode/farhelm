# A quick-launch billing question starts on the paid action

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain other. Provider generalization preserves
Tensorlake's fresh-settings wait and question publication (`list/create_form.rs:6034–6042`) and explicit primary-answer
focus (`9244–9246`, Create and launch at `9267`). `SPEC.md:3248–3249,3281–3282` still requires a later question to start
on its harmless answer. No exact current coverage or later repair was found; the old yolo proposal remains abandoned.

## TLDR

An automatically arriving sandbox quick-launch question focuses Create and launch rather than Cancel. A fresh Enter
therefore chooses the billed action instead of the harmless default required for a question arriving later.

## Details

Bucket: **other**. Confidence: **confirmed source route for automatic quick launch**, not runtime-tested. The reviewer
suggested other. Reviewed source change: `112fd9d812e95df7f69f66c85820d76faaf89582`; the adjacent behavior predates it,
and the focus rule is introduced later at the reviewed tip.

`crates/farhelm-ui/src/list/create_form.rs:5328-5431` seeds an automatic quick launch and waits for template/host
readiness, then submits its hidden launcher. The sandbox branch awaits a fresh settings GET at 5885-5893
(`api.rs:743-754`) before revealing the billing question. Its primary button at 9046 calls the focus behavior in
`lib.rs:271-276`, choosing Create and launch. No user gesture opens that visible question when it arrives.

`SPEC.md:3221-3222,3254-3255` requires later-arriving questions to start on their harmless answer. The sandbox creation
contract at 1445-1448 distinguishes Cancel, which creates nothing, from Create and launch, which proceeds. Focus Cancel
on this automatic arrival through a policy that also respects moved-on activity.

Do not infer the same defect merely from manual Launch awaiting settings: that click can plausibly count as explicitly
opening its confirmation under the Confirm-first rule. No held, repeated or composing Enter bypass was established, and
this finding does not allege billing without a fresh affirmative answer.

The existing sandbox-launch opening-token item concerns answer identity, not initial focus. The earlier
yolo-focus-origin proposal covered plain Replace and sandbox session Suspend/Stop-and-suspend questions, not creation
billing; it has since been abandoned. Recording-time main `cb03def00870e18a98a1fd9046073d2849be3253` also removes the
broad focus TODO in anticipation of a GUI rewrite, without repairing this path. No exact current decision, Planned item,
unlanded repair, accepted bug or filter cover was established.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `async-billing-primary-focus`; `g020/l2`. The original
broader wording and synthesis narrowing remain in `prep/groups/g020/findings.jsonl`.
