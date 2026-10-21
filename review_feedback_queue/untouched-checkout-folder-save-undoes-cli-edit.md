# Untouched checkout-folder Save undoes a CLI edit

Reviewed commit: 070d81976702994e4cfc7c372965d76a749ca631

## TLDR

Leaving Settings open, changing the all-hosts checkout folder through the CLI, then pressing Save without editing the
field can restore the old folder or clear the new setting for inheriting hosts.

## Details

Suggested bucket: **high**, for durable loss of a supported concurrent configuration change. Confidence: **confirmed
source overwrite mechanism**; runtime frequency was not measured. No direct data, credential or process loss is claimed.
The same mechanism remains at pinned tip `e7d3bb107618dccd17b6a06ee807a72cb3f87ef0`; this is a pre-existing surface
observed while reviewing the checkout configuration work, not a claim that the reviewed serialization change introduced
the stale draft.

`crates/farhelm-ui/src/checkout_root.rs:33–39` initializes the draft once. Save is enabled for untouched drafts and
submits that stored value (`46,78–89`). At the pinned tip, `api.rs:602–607` serializes it directly and helm
`checkout_config.rs:693–706` sets or clears it without a baseline check. Open Settings with no global folder, set a
valid folder through the CLI, then save the unchanged blank field: `root:null` clears the acknowledged CLI setting. No
narrow timing window or second GUI is required.

`SPEC.md:3265–3271` at the pinned tip supports concurrent CLI operations and forbids GUI writes undoing their changes.
Display-lag acceptance does not cover this durable write. Edited-field replacement and edited-only whitespace trimming
are separate contracts; this record concerns a field the user did not change. Avoid emitting an unchanged stale value or
otherwise preserve the intervening write.

Possible cover: `SPEC_impl.md:255–256` generically retains separate-client last-writer-wins behavior. It does not
clearly settle this against the specific CLI-preservation requirement. The historical
`pending-folder-save-overwrites-newer-save.md` case concerns request overtaking after remount/lost replies, rather than
this later untouched-baseline Save. Proposed drop: none.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, discovery `g117/f6`; independently confirmed by
`g156`.
