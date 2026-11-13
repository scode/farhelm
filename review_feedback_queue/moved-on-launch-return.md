# A failed launch reopens over work the user moved to

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Retain high. The moved source still gates
`list/create_form.rs:2071–2120` only for naming; ordinary failures pass no baseline at `6105,6579,7208`.
`SPEC.md:1348–1352,3240–3246` retains the notice requirement, and `SPEC_impl.md:1179–1185` still calls this
implementation lag. No exact later repair or cover was found. The earlier recording-main statements below are
historical.

## TLDR

A failed launch or follow-up question can reopen the launcher and take the keyboard after the user has switched sessions
or started typing elsewhere. It should wait behind a notice the user opens.

## Details

Bucket: **high**. Reviewer confidence: **confirmed by source**, with no runtime reproduction. Original product reviewers
suggested high; supporting test reviewers suggested high or other for their test manifestations. No security, data-loss
or process-loss route is claimed.

Reviewed source change: `112fd9d812e95df7f69f66c85820d76faaf89582`. The behavior followed the automatic-return contract
when that change landed. The later reviewed tip introduces the notice-based focus rule and acknowledges that the UI is
behind; this is a gap against that final target, rather than a claim that the earlier patch violated its original rule.

At the reviewed tip, `crates/farhelm-ui/src/list/create_form.rs:1951-1997`, `admit_composer_return` checks activity only
when supplied a naming baseline. Failure and question paths pass `None`: ordinary create/Replace with at 7056-7090,
sandbox create at 5955-5990, Restart with at 6417-6459, and wake/preparation return through 2004-2014 and 6145-6161.
They reserve modal admission once other dialogs leave. The presentation effect at 5447-5473 restores isolation and
opening focus through 2055-2072. Closing a newer dialog can therefore release that old return even though the
intervening interaction advanced the activity count.

The acted-on session's claims settle before this wait. The local launcher token, draft and pill remain; switching views
does not retire that submitted owner. Those protections keep the destination and retry identity stable, but do not
authorize taking the page back. Successful New/Clone selection and naming have separate activity checks; preserve them.

`SPEC.md:1326-1331` and “Keyboard focus”, “Who may move focus” require a notice after intervening input and no automatic
dialog opening. `SPEC_impl.md:1172-1177` explicitly identifies this gap and directs using the specification as the
target. That acknowledgment is not acceptance. At recording-time main `cb03def00870e18a98a1fd9046073d2849be3253`, the
broad asynchronous-focus TODO has been removed in anticipation of a GUI rewrite, without repairing this path or changing
its product contract. Its earlier Near term presence was not Planned coverage. No exact current queue, decision,
unlanded plan, known-bug or filter cover was established. Whether an unopened waiting notice should keep new launches
refused is expressly unsettled at `SPEC.md:1328-1329`; this finding does not choose that policy.

The regression tests encode the same old return and need to change with the product:

- `e2e/tests/launch-progress.spec.ts:168-187,305-324` requires modal return/focus after switching sessions or opening
  and closing the switcher; `restart-replace-progress.spec.ts:331-357` does likewise after selecting another session.
- `e2e/tests/nonblocking-launch.spec.ts:198-208,318-326,536-542,860-866,1107-1116,1168-1173` requires return after newer
  switcher, Rename or menu interactions.
- `e2e/tests/tensorlake-sandbox-sessions.spec.ts:167-177,240-249` requires modal return after selecting another session;
  `tensorlake-lifecycle.spec.ts:631-679` supplies the related lifecycle-return manifestation.

These assertions were faithful to the earlier contract and need not fail today. Preserve their useful settlement and
readiness coverage while changing return admission. Bind presentation to the initiating activity and retain the draft
behind a notice when that activity is no longer current.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`; canonical `failed-launch-reopens`; records
`g016/f1`, `g018/l1`, `g019/l1`, `g020/f1`, `g021/l1`, `g023/g023-l1`, `g024/l1`, `g025/l1`, `g026/f1`. Original
histories remain in `prep/groups/<group>/findings.jsonl`; they are corroboration of one defect.
