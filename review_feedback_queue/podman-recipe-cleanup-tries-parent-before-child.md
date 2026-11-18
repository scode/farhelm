# Podman recipe cleanup can get stuck removing the parent image first

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Deleting an unused container recipe can keep failing because Farhelm tries to remove an image that its own second image
still needs. Retrying can make the same attempt every time.

## Details

Bucket: **high**. GUI migration final whole-tree review, R9 Sol recipe image order.

Source references: `crates/farhelm-helm/src/environment/podman/build.rs:325, 369, 694, 742, 785` at the reviewed commit
(main `crates/farhelm-helm/src/environment/podman/build.rs:325, 369, 694, 744, 793`).

The prepared commit can be a child of the software image used to run the preparer. Recipe Delete queues the software
image first and the prepared image second. Retirement stops on the first Podman removal error. When Podman refuses a
parent with a child, the pass never reaches the child. Retained recipe artifacts can also block removing or retargeting
the place.

Sol rated this likely and noncritical. There is no independent verification. The append order and first-error exit are
source-confirmed. The dependency between the two image records in a real Podman store remains a runtime premise. The
interrupted-commit queue item concerns a missing tag and uses a different failure path. SPEC_impl.md's non-forcing
removal protects in-use images, while avoiding a refusal from a recorded owned child is still possible.

Remove the owned child before the parent, or retry dependency refusals after other owned images are cleared. Keep
--no-prune and respect user-owned or in-use dependencies. Test a prepared image with its recorded software parent.
