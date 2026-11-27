# Shift+F10 on a marked row opens the combined marked menu, not the row's own menu

Reviewed commit: c1163ff06cd61d3aacae8b8e65c0aca51ee1bdd7

## TLDR

With two or more sessions marked, pressing Shift+F10 on one of the marked rows opens the menu for all marked sessions
(Restart/Replace, Stop and Delete for the whole batch) instead of that row's own menu, which is what SPEC.md and the
code's own overview say the key opens. A keyboard user expecting the one-session menu gets the batch menu; with session
confirmations skipped, its Delete deletes every marked session shown without asking. The menu is labelled ("2 marked"),
so the user still chooses from what was drawn.

## Details

Provenance: trailing review through `dbb153ec`, group g10 (session context menu), finding f3 (revised in synthesis);
group g11 (what the menu acts on), candidate c5, agreed from the row-routing side. Suggested bucket: `other`.
Confidence: confirmed for the code behavior; which reading of the spec is meant is for the maintainer.

- SPEC.md, "Session list" (added in this commit): "Shift+F10 on a focused row or group header, or on a control inside
  it, opens its menu beside the row, where ⋯ opens it." The combined menu is given only for a press: "With two or more
  marked rows displayed, a right press on a marked row opens a combined menu instead".
- The `menu.rs` module docs, "Right-click" (`crates/gui/farhelm-gui/src/session_list/menu.rs:17-26` at the tip), say
  Shift+F10 "opens the same menu as its ⋯ button" and "a right press on a marked row opens the marked menu instead".
- The row's and header's key handlers (`row.rs` ~563-569, `groups.rs` ~357-363) call
  `SessionList::open_context_menu(key, None, ..)`, and `open_context_menu` (`menu.rs:443-448`) takes the marked branch
  on `shown.len() >= 2 && shown.iter().any(|marked| marked == key)` regardless of whether `at` is `None`. This is
  deliberate in code: `open_marked_context_menu`'s docs say "(`None` for Shift+F10, beside the row's ⋯ button)", and
  `the_rows_button_closes_a_marked_menu_opened_from_its_row` opens the marked menu with Shift+F10. Group headers can
  never reach the combined menu.

Ambiguity recorded by the reviewer: the SPEC.md Shift+F10 sentence can also be read loosely as "opens whatever menu the
row's right press would, placed where ⋯ opens it". Under that reading the code is intended and the fix is a SPEC.md
clarification plus the `menu.rs` overview; under the literal reading the code deviates. Either way the overview says
something the code does not do.

Fix: amend SPEC.md and the `menu.rs` module docs to say Shift+F10 on a marked row with two or more marks shown opens the
combined menu, as the right press does; or pass the opening kind into `open_context_menu` and take the marked branch
only for a press.

Bucket note: the delete-all-marked consequence needs the user to misread a correct, labelled menu, which
`review_feedback_queue/AGENTS.md`, "Priority buckets", does not count toward `highest`; it would move up only if the
combined menu were shown to be indistinguishable from a row menu.

Possible cover: none known.
