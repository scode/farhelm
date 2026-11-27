# The existing-environment checkout note is unreachable, but SPEC.md still promises it

Reviewed commit: 40fd995bb50681aaf7a437f25390282531a15544

## TLDR

Spec and dead code only: the launcher no longer has any way to start a fresh managed checkout inside a session's
existing sandbox or container, so the warning written for that case (the folder is removed permanently, not moved to the
trash, when its last session is deleted) can never appear. The product spec still says the launcher shows it. Nobody
sees a wrong message today; the risk is a future change that brings the path back while trusting the spec and the
leftover code, which no test covers any more.

## Details

Provenance: trailing review through `dbb153ec`, group g12 (Replace with keeps the source's folder, code side), finding
f1 (synthesis). Group g13 (the tests side) first reported the removed coverage of this note as a test gap (f1), then
withdrew it on finding the note unreachable; its candidate c4 records the same dead branch. Suggested bucket: `other`.
Confidence: confirmed from source for the GUI paths listed.

- `checkout_lifetime_note` (`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1116-1129`, called only from
  `view.rs:2762` while `draft.checkout` is set) picks `EXISTING_ENVIRONMENT_CHECKOUT_LIFETIME` (`checkout.rs:89`) when
  `draft.target` is `None` and the chosen host is session-owned.
- Before 40fd995bb5 only Replace with could reach that state. It now holds Replace with's destination (`draft.rs:741`
  holds `Field::Destination`; `choose_destination`, `choose_repository` and `edit_repository_query` return early at
  `draft.rs:1279`, 1304, 1321), so it never has a checkout.
- No other mode lands on a session-owned host without a new-environment target: New takes `open_host`, which filters out
  session-owned hosts (`entry.rs:582-588`), or the local host; Clone of an environment session gets the filtered
  fallback plus a new-environment target (`entry.rs:232-245`), so it shows `ENVIRONMENT_CHECKOUT_LIFETIME`, which is
  correct; the host picker offers a session-owned host only when it is already chosen (`host_picker.rs:129`); search
  host rows refuse them (`launcher/search.rs:596`, `view/search.rs:474-479`); templates resolve only non-session-owned
  hosts (`templates/apply.rs`, `known_hosts`); Restart with holds everything.
- SPEC.md:1857-1858, "Managed checkouts": "The launcher says the same for a fresh checkout inside an existing
  session-owned environment." The commit edited the neighbouring paragraph but not this sentence. It also deleted the
  only test and golden that drew the note (`launcher-replace-checkout-in-sandbox`, and a `replace_with_tests.rs` case).

Fix: delete or reword the SPEC.md sentence (no launcher path makes such a checkout), and drop the existing-environment
branch and its constant, or keep them with a comment naming them as a guard for a future path (and then give that path a
test). Verify by grepping callers of `checkout_lifetime_note` and the host-seeding sites above.

Not checked: `launch::open_waiting` restoring a pre-change Replace with draft (in memory only, so it cannot survive an
upgrade).

Possible cover: none known.
