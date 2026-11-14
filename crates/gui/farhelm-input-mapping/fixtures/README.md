# Input fixture provenance

NOTE: These tables contain DOM events, including deliberately inconsistent key/code/keyCode combinations. A passing DOM
policy or encoder test is not proof that GPUI can represent those events.

The supervisor's two byte contracts are not copied here. The tests read them in place, so this crate is always checked
against the tables the supervisor's encoder is checked against:

- `crates/farhelm-supervisor/tests/fixtures/grid-keys.jsonl`: 1,578 key rows (plus header lines).
- `crates/farhelm-supervisor/tests/fixtures/grid-mouse.jsonl`: 768 mouse rows.

The old web client's two tables are copied unchanged from reference commit `24bc058066fd57848e788b465719b0e6da2f98f7`,
because their original directory goes away with the old GUI. Only their header comments changed since, to stop naming
the deleted generator:

| File                    | Original path                                                     |
| ----------------------- | ----------------------------------------------------------------- |
| `keydown-actions.jsonl` | `crates/farhelm-ui/js-tests/fixtures/input/keydown-actions.jsonl` |
| `move-to-cell.txt`      | `crates/farhelm-ui/js-tests/fixtures/input/move-to-cell.txt`      |

The client-action table has 1,730 event rows plus pattern definitions and headers. The Alt-click table has 1,600 seeded
screen/move cases. Its application-mode prefixes are normalized to arrow directions in the mapper test because each
native arrow is a Key message and the supervisor chooses its current encoding.

The policy port comes from the old client's `grid-input.js`; Farhelm attributes its keyboard decision, coordinate
rounding, deduplication and wheel behavior to xterm.js 6.0.0 (MIT), and the complete notices are in this crate's
`LICENSE-PORTED`. No GPL Zed widget or terminal code was copied.
