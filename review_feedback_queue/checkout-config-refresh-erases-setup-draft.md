# Configuration refresh can erase the checkout-folder draft

Reviewed commit: f2eca4357f00ae9802aa3db6b0c865ad8110198a

## TLDR

While entering a checkout folder in the launcher, an unrelated configuration change can make the field disappear and
return empty, losing the entry and focus.

## Details

Suggested bucket: `other`; unsaved form input is explicitly excluded from `highest`. Confidence: likely. Component-local
draft ownership and the unmount condition are explicit; browser lifecycle behavior was not reproduced. Source: separate
adversarial trailing reviewer. Proposed drop: none.

Select a repository with no root configured, then enter an unsaved path in its setup field. Change a post-clone setting
or another host's root through the CLI or another client. These advance the global epoch
(`crates/farhelm-helm/src/checkout_config.rs:23–27,532–574`) while the selected host still needs setup.

The new revision filter makes `observed_root_required` unknown and `needs_checkout_root` false
(`crates/farhelm-ui/src/list/create_form.rs:3856–3869`). Changed preview generation rejects the previous discovery
answer while a new request runs. `show_checkout_root_setup` becomes false (`3953`), unmounting `CheckoutRootField` at
`6001–6012`. That component owns its draft locally (`checkout_root.rs:26–38`); remount initializes from saved
configuration instead of preserving the unfinished entry. Keep an existing editor mounted during eligibility refresh,
retiring it after a current answer establishes that setup is unnecessary.

Possible cover: SPEC.md, “Managed checkouts,” requires refused drafts to remain editable; SPEC_impl.md's GUI section
describes retained draft behavior. The completed inline-setup ledger outcome fixes bringing setup back after removal,
not preserving an already-visible editor. The CLI is a supported concurrent surface under “One GUI at a time”; this does
not require multiple GUI support. No matching acceptance, Planned item, BUGS entry, queue item or filter was found.
