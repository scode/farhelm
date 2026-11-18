# A template collision message can render its name out of order

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

When a name is already taken, the refusal can display a right-to-left name differently from how Farhelm shows it
elsewhere.

## Details

Haiku pass H-P8-3, checked by gpt-6.2-sol. Suggested queue bucket **other**. Confirmed shared host-text formatting is
bypassed. No wrong-target action was established.

- Verdict: NOT CRITICAL
- Route checked: `templates/editor.rs:137–144,689–724`, `templates/fields.rs:471–506`, `templates/dialog.rs:662–669`,
  and `SPEC.md:4964–4968`; same Debug formatting at browser `list/templates.rs:953` in `24bc0580`.
- Reason: The refusal interpolates the proposed draft name, after matching it exactly against stored names. Rust Debug
  quotes and escapes nonprinting characters, but it does not provide Farhelm's shared host-text spelling or isolate
  ordinary RTL. The message refuses the save before dispatch, and no wrong-target or disclosure route is established.

Build the refusal as a sentence with the draft name as a separately escaped and direction-isolated value, as Undo does.
Test a collision containing directional text.

Possible cover: the existing `template-cached-collision-blocks-freed-name.md` and its triage decision concern stale name
availability. This is the rendering of the refusal even when the collision is current.
