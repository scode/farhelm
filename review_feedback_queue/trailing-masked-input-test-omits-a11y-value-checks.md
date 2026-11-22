# Masked-input test omits accessible value checks and never runs with accessibility active

Reviewed commit: e2a3aee42941f801279e29980acd18cbce26e7c7

## TLDR

The masked-input test verifies that a typed secret is absent from painted text and accessible names, but never checks
the accessible value field or runs with accessibility active, leaving the screen-reader value path unguarded against
regressions.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/components/tests/text_input.rs:189`.

In commit e2a3aee42941f801279e29980acd18cbce26e7c7, `a_masked_input_holds_what_was_typed_and_paints_none_of_it`
(crates/gui/farhelm-gui/src/components/tests/text_input.rs:174-211) was sharpened to assert `Role::PasswordInput`,
`password.label == Some("Title")`, and that no `DriverNode.label` contains `"xyz"`. However:

1. `DriverNode` (vendor/gpui-pre/src/window/driver.rs:54-56; populated at vendor/gpui-pre/src/window.rs:4363-4366)
   carries a separate `value: Option<SharedString>` field for accessible field text (`node.value()`), which the test
   never inspects.
2. `TextInput` (crates/gui/farhelm-gui/src/components/text_input.rs:209-215,828-855) sets `.masked(true)` on
   `InputState` without setting `Input::content_type(InputContentType::Password)`. In `gpui-component` 0.9.1
   (gpui-component-0.9.1/src/input/input.rs:111-116,746-753), `exposes_accessibility_value(content_type)` checks only
   `InputContentType::Password | NewPassword`, so it passes `.with_value_exposed(true)` for Farhelm's masked fields;
   suppression currently depends solely on `!self.masked` inside `gpui-base`
   (gpui-base-0.9.1/src/input/base/state.rs:5168-5173).
3. Because `gpui-base` emits `aria_value` only when `window.is_a11y_active() || cfg!(feature = "test-support")`, and
   `farhelm-gui` neither enables `gpui-base/test-support` in dev-dependencies (crates/gui/farhelm-gui/Cargo.toml:55-65)
   nor activates accessibility in `test_support::frame` (crates/gui/farhelm-gui/src/test_support.rs:588-592), even the
   plain field's `"abc"` never appears in `node.value` during the test. Thus a future Kit/base change that dropped
   `!self.masked` (or relied on `value_exposed`) would leak masked secrets to screen readers while this test still
   passed. Fix direction: pass `InputContentType::Password` on masked `TextInput` fields so `value_exposed` is false
   too, and assert in the test (with `aria_value` emission active so the plain field's `node.value` proves `"abc"` is
   reported) that the masked field's `node.value` is `None` and no node's `value` contains `"xyz"`.

Reviewer metadata (g06-triage-gui-tests-and-bulk/l2): suggested bucket: other (cannot be highest today: in shipped code
at 66ad3aaa95ef03a2130e2d0194d58353912ae9da, gpui-base 0.9.1 suppresses aria_value via `!self.masked` at
gpui-base-0.9.1/src/input/base/state.rs:5168-5173, so no production secret leak is currently established; this is a test
assertion gap and defense-in-depth omission); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g06-triage-gui-tests-and-bulk/l2`.
