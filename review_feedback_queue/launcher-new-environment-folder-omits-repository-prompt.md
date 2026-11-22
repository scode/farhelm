# New-environment checkout status omits the repository prompt

Reviewed commit: f249fdff55a2bd63085e1f911757e61e41bd5b9f

## TLDR

When no repository has been selected for a new environment, the Folder row says cloning will happen and omits its
invitation to select a repository.

## Details

`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1254` checks for a new environment before it checks whether the
selected checkout has a repository. Both states are possible together during ordinary form entry. The first branch
replaces the unselected-repository text.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`, finding `checkout-layout/f4`.
