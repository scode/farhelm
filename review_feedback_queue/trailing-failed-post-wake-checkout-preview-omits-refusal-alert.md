# Failed or superseded post-wake checkout preview reopens the launcher without a Not launched alert

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When you launch a managed checkout on a suspended or stopped host and the checkout preview fails after the host connects
(for example, a typed session name is already taken or the host has no checkout folder configured) or is superseded, the
launcher reopens without any "Not launched." refusal alert beside Launch. Screen readers receive no alert announcement,
and a superseded preview shows no refusal text at all.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/checkout.rs:562`.

Promoted from Turn-1 candidate `c2` after checking cross-group input (`g13:f1`) against `SPEC.md:398` ("A failed
preparation says the session was not launched"), `SPEC.md:1559-1561` ("In the session launcher every refused launch
(New, Clone, Replace with, quick launch) shows as a bordered alert with a warning icon beside Launch, starting 'Not
launched.' before the refusal's own words, and announced to assistive technology"), and the reference e2e test at commit
`24bc0580:e2e/tests/tensorlake-lifecycle.spec.ts:1655-1658`
(`if (mode === "typed-taken") { await expect(form.getByRole("alert")).toContainText("a directory with that checkout name already exists"); }`).
In `crates/gui/farhelm-gui/src/launcher/view/checkout.rs:562-590`, when `settle_waiting_launch` sees `Preview::Failed`
or a superseded key (`launcher.preview_key(cx).as_ref() != Some(&armed.key)`), it sets `start = false`, takes `armed`,
and calls `launch::reveal_prepared(entity, armed.window, armed.mark, cx)` without calling `wake_refused` or setting
`self.pressed = true` / `self.note` / `self.refused`. When `Launcher::render`
(`crates/gui/farhelm-gui/src/launcher/view.rs:2559-2587`) draws the reopened launcher, `alert` is `None`, so no
`refusal_alert` (`Role::Alert`, starting with `Not launched.`) is rendered beside Launch. On `Preview::Failed`, only
`launcher-preview-error` appears inside `checkout_folder_line` (`checkout.rs:1209`, `Role::Status`, not `Role::Alert`),
and on a superseded key no error appears at all. Fix direction: In `settle_waiting_launch`, when
`armed.held_out_of_sight` ends with `!start`, record a refusal via `wake_refused` (using the failed preview's reason or
a superseded-preview explanation, prefixed with the mode's lead per `f4`) before calling `reveal_prepared`.

Reviewer metadata (g11-wake-launch-views-and-panes/c2): suggested bucket: other (could not be in highest: missing
refusal alert / accessibility announcement and spec mismatch; no session is created and Launch stays refused);
confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/gui/farhelm-gui/src/launcher/checkout_tests.rs:2560` (g13-wake-checkout-launch-tests/f1):

Introduced in commit 3f6ea3b0cf6f55f82395569cf24be27e8e18d273 and unchanged at 66ad3aaa95ef03a2130e2d0194d58353912ae9da.
In `crates/gui/farhelm-gui/src/launcher/view/checkout.rs:541-590` (`Launcher::settle_waiting_launch`), when a hidden
wake's preview settles as `Preview::Failed` (lines 565-570) or its `PreviewKey` no longer matches `armed.key` (lines
562-563, e.g. on a direct `Connected(n) -> Connected(n+1)` incarnation jump that `wake::follow` at `wake.rs:214-219`
ignores, cross-checked with `g10` `l3`), `start` is `false` and `settle_waiting_launch` calls
`launch::reveal_prepared(entity, armed.window, armed.mark, cx)` (lines 587-589) without calling
`launcher.wake_refused(...)`, setting `launcher.pressed = true`, or recording any `Note::WakeRefusal`. When
`Launcher::render` (`crates/gui/farhelm-gui/src/launcher/view.rs:2559-2587`) draws the revealed dialog, `self.pressed`
is `false`, `self.refusal_words(cx)` is `None`, and `self.note` is `None` (cleared in `submit` at `view.rs:1384`), so no
`launcher-refusal` (`Role::Alert`, bordered alert beside Launch starting with "Not launched.") is rendered. Only the
inline Folder status line `launcher-preview-error` (`checkout.rs:1209-1215`, `Role::Status`, `Live::Polite`) shows the
raw preview error string—and in the mismatched-`PreviewKey` branch even that line shows no error. Spec clarification
(cross-checked with `g11` `c2`): `SPEC.md` explicitly governs post-wake checkout preview failure with no exception.
`SPEC.md` "Topology" (lines 397-398) states: "The session launcher shows its own **Resuming** preparation through the
provider call and reconnection, including quick launch. It says the session starts when the host is ready. A failed
preparation says the session was not launched." `SPEC.md` "Sessions -> Creation" (lines 1559-1561) states: "In the
session launcher every refused launch (New, Clone, Replace with, quick launch) shows as a bordered alert with a warning
icon beside Launch, starting 'Not launched.' before the refusal's own words, and announced to assistive technology." The
doc comment in `checkout.rs:533-534` ("A quick launch that does not start comes back to be seen, with the failed
preview's line or the new preview") is an implementation comment carried over from pre-wake non-wake quick-launch
preview handling (cf. `checkout_tests.rs:2268`), not a `SPEC.md` rule, and conflicts with `SPEC.md:398,1559-1561` once
`#4573` reused `settle_waiting_launch` for woken checkouts (`checkout.rs:519`). Test assertion gaps: (1)
`a_woken_checkout_with_a_taken_typed_name_returns_to_the_form`
(`crates/gui/farhelm-gui/src/launcher/checkout_tests.rs:2557-2581`) claims to re-show the form "with the refusal" (and
`docs/gpui/parity/launcher.md:1264` claims parity with reference `24bc0580:e2e/tests/tensorlake-lifecycle.spec.ts:1656`,
which asserted `form.getByRole('alert')`), yet only asserts `name_of(cx, "launcher-preview-error")` and never checks
`shown_refusal(cx)`. (2) `a_woken_checkout_shortcut_returns_if_the_host_drops_out_during_preview`
(`crates/gui/farhelm-gui/src/templates/dispatch_tests.rs:923-949`) only tests `Connected -> Unreachable` (handled by
`wake::follow`), missing a direct `Connected(n) -> Connected(n+1)` incarnation change that hits
`settle_waiting_launch`'s silent mismatched-key reveal path. Verification/fix direction: in `settle_waiting_launch`
(when `armed.held_out_of_sight` and `!start`), record a `Note::WakeRefusal` starting with "Not launched." and the
preview error (or a stale/superseded preview explanation) before calling `reveal_prepared`; assert `shown_refusal(cx)`
in `a_woken_checkout_with_a_taken_typed_name_returns_to_the_form` and add a test for a direct incarnation change during
post-wake preview.

Reviewer metadata (g13-wake-checkout-launch-tests/f1): suggested bucket: other (cannot be in highest: no security,
credential, process, or user-data loss); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g11-wake-launch-views-and-panes/c2`,
`g13-wake-checkout-launch-tests/f1`.
