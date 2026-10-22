# A lost shortcut-save reply can restore an assignment the next launch does not use

Reviewed commit: 44051162a377d1d27360694d5ced54618cf12eb4

## TLDR

If a shortcut reassignment is saved but its reply is lost, Templates restores the earlier assignment. After the dialog
closes, that key can launch the newly saved template instead. The mismatch is source-established; harmful reliance on a
different valid template, and loss of user work, remain unproved.

## Details

The helm commits bindings before constructing the response (`crates/farhelm-helm/src/store.rs:5104–5105,5154,5208`;
`preferences.rs:249–254`). A lost reply returns failure to the client, which discards its overlay and restores the
pre-save map (`crates/farhelm-ui/src/api.rs:1399–1418`; `list/templates.rs:1504,1533–1536`). The dialog describes the
shown chord as set and as starting that template immediately (`templates.rs:347–385,1897–1911`).

Shortcuts yield while Templates is open. Once it closes, dispatch freshly resolves the stored binding, so it can select
the newly saved template rather than the restored indication (`list/view.rs:3559–3589,3624–3637,3862–3867`). Automatic
submission retains ordinary readiness and admission checks
(`list/create_form.rs:669–672,3756–3842,4554–4561,4660–4671`). Templates can differ in command, host and placement
(`farhelm-proto/src/launcher.rs:233–299,831–860`); no bypass of those checks is alleged.

Suggested bucket: **provisionally highest** because a wrong intended launch could affect user work; otherwise high for
misleading launch indication. Confidence: **confirmed conditional indication/dispatch mismatch**, not a demonstrated
harmful launch. Human reliance and a materially harmful valid configuration remain missing impact premises.

Possible cover: `SPEC_impl.md:241–250` accepts unknown remote outcomes; `SPEC.md:3347–3350` and
`TRIAGE_OUTCOMES.md:17676–17684` require discard and describe rollback. Independent review could not unambiguously
extend those rules to this misleading launch indication; desktop unknown-outcome reporting remains required at
`SPEC.md:3308–3314`. Cached-label acceptance has a different trigger. No exact Planned, BUGS or filter match was found.
Proposed drop: none. Resolve the rollback/indication contract before broadening the save design.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, `g010/f3` first branch, `a007/f1`, `g075/f4`. The final landing report at
`9394188108a1f5e0249fb4f2dd81253d76690be6`, `plans/reports/gui-saves-last-write-wins.report.md:162–165`, prompted the
interface check. No runtime reproduction. The immediate pre-commit 503 regression does not exercise a lost success
reply.
