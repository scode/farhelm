# The Claude login recreation test clears the stamp it should distinguish

Reviewed commit: aebf561680cc851498c6619d28b1be1ed3600617

## TLDR

The new regression for deleting and recreating a Claude login would pass even if delivery stopped distinguishing the old
login from a new one with the same name and revision. Production currently includes that identity check; this is a gap
in regression protection.

## Details

Suggested bucket: **other**. Confidence: confirmed source and interface checks; no runtime experiment. Unchanged at
trailing-review tip `49ce8cb76e9f4c841e15122a89264c7c908171dc`.

`crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:348–350` completes a None delivery before recreating
the login at `353–364`. Production resolves None to `Cleared` (`credential_delivery.rs:638–644`) and commits that stamp
over the previous `Subscription` (`496–501`). The later comparison at `412–435` sees different variants whether or not
the new Subscription includes creation identity.

Production currently constructs the full name/revision/incarnation Subscription (`669–691`), as SPEC_impl.md, Named
credential catalog, requires. No missing production identity check is alleged. The endpoint recreation test
(`credential_delivery_tests.rs:569–623`) exercises its Endpoint variant; the Codex test (`436–480`) protects its
separate retained authentication tuple (`credential_delivery.rs:503–550`). Fresh-host setup performs first delivery and
supplies no alternative Claude recreation witness retaining the old Subscription stamp.

A distinguishing witness would release the recipe reference without delivering None, recreate revision one under the
same name, restore the selection and reconnect while retaining the old Subscription stamp. The existing test still
usefully checks recreation after a completed None connection. No exact accepted omission, planned repair, queue item,
ledger or filter coverage was found.

Provenance: run `4ca8e296-4f73-4429-a34b-031af1582085`, `g011/g011-claude-incarnation-oracle`, corroborating
`g010/g010-claude-incarnation-interface` and `g006/g006-006`. These represent one test gap. Proposed drop: none.
