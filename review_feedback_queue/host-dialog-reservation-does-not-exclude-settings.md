# Host-dialog reservation does not exclude Settings

Reviewed commit: a0013a0cfc5aaf839450a4f08fa696ff4a675573

## TLDR

Opening Settings while a delayed host question is being admitted can leave both dialogs ignoring clicks and ordinary
keys. Escape remains available to recover.

## Details

Suggested bucket: **other**. Confidence: **likely, conditional on competing input ordering**. The separate asynchronous
stages and missing shared guard are established; native manifestation and frequency were not measured.

`crates/farhelm-ui/src/modal_isolation.rs:83–98` reserves a DOM marker against other host questions, awaits the
evaluation result, then mounts its child without rechecking ownership. The Settings gear opens Settings without
consulting that reservation (`app_bar.rs:707–717,778–780`). Matching desktop Dioxus 0.7.10 source separates the
evaluation reply, channel wake, Rust rendering and DOM-edit delivery. A gear click can therefore compete while the
reserved host child is still absent.

If both dialogs mount, their installers can make each other's branches inert (`modal_isolation.rs:164–180,198–207`). The
regression at `e2e/tests/provisioning.spec.ts:451–460` opens Settings before releasing the reply and does not exercise
reservation followed by Settings. Document-level Escape clicks enabled cancel controls (`183–195`), whose close paths
remain available; release removes owned inert attributes (`210–224`). The stronger permanent-lock claim is refuted. No
automatic host operation, wrong-target mutation or user-work-loss consequence is established.

Possible cover: the subsecond person-action filter may apply, but the bridge-to-mount interval's width is unestablished.
`SPEC.md:3273–3276` accepts some delivered late input; it does not clearly accept mutual inertness. The existing
`add-host-refusal-bypasses-dialog-admission.md` concerns a different refusal/reopening path. No exact coverage was
found. Proposed drop: none pending that filter ambiguity.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, discovery `g039/f4`.
