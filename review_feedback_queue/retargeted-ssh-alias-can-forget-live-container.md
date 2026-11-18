# A retargeted SSH alias can make Delete forget a live container

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

If an SSH alias starts reaching a different machine or login account, Farhelm can report the old container deleted and
remove it from Hosts even though it is still on the original machine.

## Details

Bucket: **high**. GUI migration final whole-tree review, R9-4.

Source references: `crates/farhelm-helm/src/access.rs:436–448`,
`crates/farhelm-helm/src/environment/podman_probe.rs:393`,
`crates/farhelm-helm/src/environment/podman.rs:157–190, 405–419, 1396–1426` at the reviewed commit (main
`crates/farhelm-helm/src/environment/podman.rs:184–217, 432–446, 1411–1441`), `crates/farhelm-helm/src/hosts.rs:1389`.

Create a container through a registered SSH alias. Change the alias outside Farhelm and let the old connection go away.
On the new Podman endpoint, the old canonical ID returns absent. The cached route compares the registered destination
string and SSH configuration path. The provider runs raw Podman commands without requiring the ordinary supervisor's
identity, so absence on the replacement endpoint supplies the proof that lets the helm drop the old host and its cached
sessions.

Sol rated this source-confirmed and critical. Opus verification R9-4 confirmed it and rated it noncritical: the old
container and work survive, although they lose Farhelm's management route. High reflects falsely successful deletion and
lost management. The queue's podman-storage-selection-escapes-lifecycle-lookup.md concerns two Podman stores selected by
shell environment in the retired supervisor runtime. It does not address remote SSH endpoint/account changes. SPEC.md's
logical host entry rule may cover an operator-approved retarget, while this route changes the endpoint externally and
deals with prior place-owned resources.

Record the Podman place's machine and account identity when binding the resource. Refuse to confirm absence under
another identity. Test the live old container and the absent lookup after retarget.
