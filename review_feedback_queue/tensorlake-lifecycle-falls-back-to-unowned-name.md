# Suspend or Resume can reach a different Tensorlake sandbox

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

If Tensorlake setup refuses a sandbox name already in use, the failed Farhelm host can still offer Suspend or Resume and
act on the existing sandbox.

## Details

Bucket: **high**. GUI migration final whole-tree review, R9-2.

Source references: `crates/farhelm-helm/src/tensorlake/hosts.rs:394–401`,
`crates/farhelm-helm/src/tensorlake/lifecycle.rs:132, 182–210`,
`crates/farhelm-helm/src/provisioning/tensorlake_add.rs:116–135, 169, 736`,
`crates/gui/farhelm-gui/src/hosts/offers.rs:376–386`.

The setup confirmation shows the generated sandbox name. If an independently created sandbox already occupies it when
setup runs, create refuses adoption without a saved ID or pending send. The failed host can still offer Suspend. The
lifecycle path falls back to the name without those ownership facts. It continues using that string after the first GET
returns a canonical ID, so it can also address a later same-name sandbox.

Sol called this source-confirmed and critical and supplied the same-name setup route. Opus verification R9-2 confirmed
it and rated it noncritical: a reserved-name collision is required, the change shipped before the migration, and
Suspend/Resume is reversible. High records the wrong sandbox's state and billing change. SPEC_impl.md's pending-create
recovery permits certain name lookups, but this route has neither a pending marker nor an ID.

Refuse lifecycle operations when both identity facts are missing. When recovery is permitted, address the canonical ID
returned by the lookup. Verify refusal on the failed row and identity stability after a successful lookup.
