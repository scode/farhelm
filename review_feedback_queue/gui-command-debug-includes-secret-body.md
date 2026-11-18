# Debug formatting can reveal secrets in GUI commands

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A future diagnostic that prints a GUI command could expose a token or login file included in that request.

## Details

Bucket: **other**. GUI migration final whole-tree review, R6 Opus 3.

Source references: `crates/gui/farhelm-gui/src/operations.rs:453–462`, `crates/farhelm-proto/src/ui_channel.rs:80–81`,
`crates/gui/farhelm-gui/src/settings/tensorlake_key.rs:66–74`,
`crates/gui/farhelm-gui/src/settings/credentials.rs:98–126`.

The GUI Command derives Debug over its optional JSON body. That erases the typed boundary used by the protocol's
secret-bearing requests. Tensorlake, named credentials and ntfy operations supply secret-bearing bodies. SPEC_impl.md
“Named credential catalog” records why the request types themselves lack Debug.

Opus confirmed the derive and rated it noncritical. The reviewer found no shipped logging call that formats a
secret-bearing command, so the exposure is latent. Other is appropriate until an actual formatting sink is found. A
shipped sink that emits secrets would move this to highest.

Remove the derive or implement Debug without the body. Check formatting with a placeholder secret.
