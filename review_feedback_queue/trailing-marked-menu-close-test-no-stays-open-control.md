# Test gap: the marked right-press menu's close test would pass if it closed on every update

Reviewed commit: c1163ff06cd61d3aacae8b8e65c0aca51ee1bdd7

## TLDR

Test tooling only: the test that guards when a right press's combined menu for marked sessions closes would still pass
if that menu closed on every update the helm sends (an activity change, say), which would make it vanish under the
user's pointer while agents are working. Nothing else pins that it stays open.

## Details

Provenance: trailing review through `dbb153ec`, group g10 (session context menu), finding f2. Suggested bucket: `other`.
Confidence: confirmed from the test's source.

- `a_marked_context_menu_closes_when_its_lifecycle_actions_change`
  (`crates/gui/farhelm-gui/src/session_list/context_menu_tests.rs`) asserts only that the menu is gone after two pushes
  that change the lifecycle offer or the shown count. Its docstring says "The metadata actions must stay the same across
  each change, so that only this rule can close the menu", but without a push that changes neither, followed by an
  assertion that the menu is still drawn, the test cannot tell the rule from an unconditional close.
- The lifecycle branch is new and separate from the bar menu's rule (`marking.rs`, `marks_projected`, the
  `lifecycle_changed` condition). The bar menu's stays-open behavior is exercised in `marking_tests.rs`
  (`the_marked_menu_mutes_and_unmutes_only_the_eligible_rows` keeps the menu across a push), but only with
  `menu_lifecycle` false, so an inverted or overly broad `lifecycle_changed` would go unnoticed.

Fix: before the first changing push, push a listing that keeps the same rows and metadata offers (or changes only an
activity stamp) and assert the menu is still drawn.

Caveat: the reviewer checked `context_menu_tests.rs` and `marking_tests.rs`, not every other test module.

Possible cover: none known.
