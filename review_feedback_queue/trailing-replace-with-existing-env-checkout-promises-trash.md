# Managed-checkout lifetime note promises Trash on Delete inside an existing sandbox or container

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When you choose Replace with on a session inside an existing sandbox or container and switch Destination to Managed
checkout, the launcher tells you the new checkout 'moves to the trash when its last session is deleted', even though
deleting the last session inside a sandbox or container discards the checkout or deletes the entire environment
permanently.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1300-1307`.

In f249fdff, `crates/gui/farhelm-gui/src/launcher/view/checkout.rs:102-106` and `1300-1307` split the lifetime note into
`HOST_CHECKOUT_LIFETIME` ('New folder named after the session; it moves to the trash when its last session is deleted.')
and `ENVIRONMENT_CHECKOUT_LIFETIME` ('New folder named after the session, inside the new environment; deleting its last
session removes it permanently.'), choosing between them solely by `if self.draft.target.is_some()`. However, when
opening Replace with on a session inside an existing session-owned sandbox or container (`SPEC.md`, 'Sandbox sessions':
'Restart, Restart with, Replace and Replace with stay in the existing sandbox'), `self.draft.target` is `None` and
`self.draft.host` is that session-owned environment host (`host.is_session_owned && host.environment.is_some()`, as
checked by `SessionCheckout::of` in `crates/gui/farhelm-gui/src/session_pane/details.rs:221-223` and used by
`replacement_checkout_warning` in `checkout.rs:1014-1016`). In that mode, `checkout_lifetime_note` falls into the `else`
branch and shows `HOST_CHECKOUT_LIFETIME`, contradicting `SPEC.md`, 'Sandbox sessions' (lines 1637, 1647-1652, 1693) and
'Container sessions' (lines 5317-5320, 5334), where deleting a session in a session-owned environment either discards
the checkout rather than moving it to the checkout trash (if other sessions remain) or deletes the whole
sandbox/container permanently (if it was the last session). It also jars directly with `replacement_checkout_warning`
shown at the top of the same Destination card. Fix: pass `cx: &App` to `checkout_lifetime_note`, check whether
`self.draft.target.is_some()` (new environment) or
`self.chosen_host(cx).is_some_and(|h| h.is_session_owned && h.environment.is_some())` (existing session-owned
environment), and use appropriate permanent-removal wording for existing environments (e.g., 'inside this environment'
rather than 'inside the new environment'). Note on other sub-checks bundled in lead l5 (both refuted): (a) grep
confirmed no remaining callers/tests reference removed `launcher-checkout-folder` or `launcher-preview-heading` (only
negative assertions in `checkout_tests.rs:514-515, 2327`), and (b) `draft.holds(Field::Destination)`
(`draft.rs:568, 665-671, 1157-1205`) is only true in `Mode::RestartWith`, where `draft.checkout` is always `None` and
cannot be toggled on.

Reviewer metadata (g24-launcher-compact-checkout/l5): suggested bucket: other (could not be in highest: although this
note misstates Delete behavior, actual Delete of a session in a sandbox or container always opens a non-skippable
confirmation explicitly stating that the checkout / sandbox or container is permanently deleted per SPEC.md 'Sandbox
sessions' and 'Container sessions', so no unwarned data-loss route is established); confidence: confirmed; possible
cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g24-launcher-compact-checkout/l5`.
