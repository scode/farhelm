# A completed setup save can steal focus from a newer input

Reviewed commit: 19f967247150131de3b37127df7f028669ffe269

## TLDR

When a default checkout-folder save finishes after the user changes to a new environment and starts editing another
field, the launcher can move focus back to Repository.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1145`.

checkout.rs:334–338 and 369–376 make setup unavailable for recipe placement, but lines 1125–1149 keep the checkout field
and its subscription alive. settings/checkout_field.rs:491–502 emits CheckoutUpdated on successful save and
checkout.rs:1145 takes Mark when that result arrives. The setup-hidden branch at checkout.rs:1125–1134 then requests
repository focus. launcher/view/host_picker.rs:376–399 and 470–478 leave host selection available when not held by the
draft, regardless of the checkout save. focus.rs:271–295 and 621–639 check only input since that late mark and the
current layer, so an intervening host change and edit within the launcher do not themselves block the move.
SPEC.md:2032–2033 forbids late replies stealing focus. Take the mark when Save starts and check the same rendered
placement before the focus handoff. The focus logic is also present in the exact parent
1e36b2ac090eeaa70c280c3ab4d58cbbc08bbe2f (checkout.rs:1124–1149). The newly added environment transition test at
checkout_tests.rs:1689–1730 supplies a discovery reply but never starts a setup save. Source-supported, no observed
focus change. Bucket: other. Confidence: confirmed by source. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `checkout/l9`.
