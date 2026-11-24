# more_options locks the disclosure open as needing attention while hiding the conversation field when a custom agent disables resumption in Restart with

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

In Restart with for a custom-agent session that has no captured conversation, if the custom agent's registration has
resumption disabled (or is deleted), the More options disclosure is locked open with "A setting here is required or
needs attention", yet shows neither the conversation field nor any error inside it.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/more_options.rs:33-43`.

In `crates/gui/farhelm-gui/src/launcher/view/more_options.rs:33-43`:

```rust
let offered = harness.filter(|harness| {
    self.draft.offers_conversation()
        || (harness.uses_custom_catalog() && !self.draft.conversation.is_empty())
});
let label = offered.map(conversation_label);
let required = self.draft.requires_conversation();
let conversation_error = self.draft.conversation_error();
let wrapper_error = self.draft.wrapper_error();
let must_open = required || conversation_error.is_some() || wrapper_error.is_some();
```

When `Mode::RestartWith` opens with `restart.conversation_required == true` (`required == true`) on a custom agent whose
`custom_resumption` is `false` (either because resumption was turned off in Settings or the registration was deleted)
and `self.draft.conversation` is empty:

1. `self.draft.offers_conversation()` (`request.rs:536-541`) returns `false`, and `!self.draft.conversation.is_empty()`
   is `false`, so `offered` is `None`.
2. `must_open` is still `true` (because `required` is unconditionally included), which sets
   `self.more_options_open = true`, marks the toggle `.busy(true)` (un-foldable via `toggle_more_options` at
   `more_options.rs:213-222`), and appends `" A setting here is required or needs attention."` to its accessible name
   and hover help (`more_options.rs:146-148`).
3. Because `offered` is `None`, the entire `if let Some((harness, label)) = offered.zip(label)` block
   (`more_options.rs:44-90`) is skipped, so More options displays only the unrelated Wrapper prefix input with no
   explanation or required field.

This is a view-layer companion to `g52/f4` (`request.rs:380`): even if `restart_refusal` is fixed to refuse disabled
resumption directly rather than asking for a conversation, `more_options.rs:39` and `toggle_more_options`
(`more_options.rs:213`) will still lock More options open with nothing actionable inside unless `required` is gated on
`offered.is_some()` (or an inline status explaining why resumption is unavailable is shown).

How to verify: Inspect `crates/gui/farhelm-gui/src/launcher/view/more_options.rs:33-90,146-148,213-222` and
`crates/gui/farhelm-gui/src/launcher/request.rs:536-548`.

Fix: Gate `must_open` and `toggle_more_options`'s `requires_conversation()` check on `offered.is_some()` (or
`self.draft.offers_conversation()`), or render an inline notice when `required && offered.is_none()`.

Reviewer metadata (g53_custom_agents_gui_launcher_b/f10): suggested bucket: other — Confusing locked-open disclosure UI
state in Restart with when a custom agent without captured conversation has resumption disabled; no security or
data-loss impact.; confidence: confirmed (source analysis of `more_options.rs:33-90,146-148,213-222` and
`request.rs:536-548`; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g53_custom_agents_gui_launcher_b/f10`.
