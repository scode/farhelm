# RadioGroup chip uses flex_none() without max_w_full() or truncation, allowing long custom-agent display names to overflow the launcher harness row

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

A registered custom agent with a long display name produces a chip in the session launcher's Custom group that cannot
shrink or ellipsize to the dialog width, overflowing and getting clipped at the right edge of the launcher.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/components/radio_group.rs:1022`.

Commit df0dff4c9b925534b0aae763c325155279a58829 extended `RadioGroup` (`Look::Chips`) with `RadioOption::catalog_label`
and `RadioGroup::separated_named_chips` (crates/gui/farhelm-gui/src/components/radio_group.rs:197-213,416-429,817-843)
for the session launcher's Custom agents chip group (`crates/gui/farhelm-gui/src/launcher/view/choices.rs:355-380`).
While `separated_named_chips` adds `.flex_wrap().min_w_0()` to the `launcher-custom-row` container (radio_group.rs:839)
so multiple chips can wrap, each individual chip in `OptionState::chip` (radio_group.rs:1017-1050) still sets
`.flex_none()` (`flex-grow: 0; flex-shrink: 0`) with no `.max_w_full()`, no `.min_w_0()`, and no `.truncate()` on the
chip or on `option.label_element()` (`contents()` at radio_group.rs:1214). Because `CustomAgent::validate`
(crates/farhelm-proto/src/custom_agents.rs:25-33) imposes no maximum length on `display_name`, a custom agent whose
display name is wider than the launcher dialog cannot shrink or ellipsize inside `launcher-custom-row` and extends past
the dialog's right edge. How to verify: inspect components/radio_group.rs:839,1017-1046,1214 and
launcher/view/choices.rs:355-380,403-407. Fix: constrain chips with `.max_w_full().min_w_0()` (and wrap
`option.label_element()` in `.min_w_0().truncate()` when `option.host_label.is_some()`, or apply `.truncate()` on
single-line chip labels so the focus ring and icon stay intact while long labels ellipsize cleanly inside the chip).

Reviewer metadata (g51_custom_agents_gui_settings_b/f7): suggested bucket: other (layout overflow / clipping in the
launcher chip picker for long custom-agent display names; no security or data-loss impact).; confidence: confirmed;
possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g51_custom_agents_gui_settings_b/f7`.
