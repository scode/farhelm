# Host-removal test leaves its confirmation preference changed

Reviewed commit: a03f32543bdaed2cbfea9cf33d43335611e071ff

## TLDR

The new host-removal test leaves the shared test helm's confirmation preference overwritten. Whether this changes a
later test's behavior remains unverified; it needs an inherited true value.

## Details

Suggested bucket: **other**, for private fixture isolation. Confidence: **confirmed cleanup omission; behavioral
contamination unverified after investigation**. No product authorization defect or user-work loss is claimed.

`e2e/tests/provisioning.spec.ts:431–454`, especially `432`, writes `skip_host_remove_confirmation: false` without saving
its inherited value. Cleanup at `1111–1124` resets setup/session confirmation settings but omits host-removal
confirmation. Even successful completion leaves the field changed. The long-lived worker-stack contract at
`e2e/playwright.config.ts:98–107` requires restoration of shared state. This remains at pinned tip
`9b19f8d754bf7658761ef246cfda0ea8397f4f21`.

An effective confirmation change requires an incoming true value: false and absent behave alike (`hosts.rs:2117`). The
reviewer inspected the other true-setting tests and their restorations, without establishing an ordinary runner history
supplying true and a later affected consumer. That history is the remaining premise. No runtime ordering was tested.
Capture the inherited field before changing it and restore it independently in `finally` or an armed cleanup hook.

Possible cover: none exact found. The existing checkout/chrome restoration item concerns another setting and a preceding
cleanup failure, while this omission is unconditional. No exact specification acceptance, Planned item, unlanded repair,
BUGS entry, queue item, ledger decision or filter was found. Proposed drop: none.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g017/g017-f1`.
