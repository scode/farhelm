# The fresh-proxy regression overlooks the installed boot file

Reviewed commit: cb4a0708448eb12956064a0cbfe8b3ac2c4f7b51

## TLDR

A regression could send the right values in the first import while writing the wrong bytes for later container boots.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman/failure_tests.rs:1445`.

failure_tests.rs:1394–1459 uses a warm cache, changes the place’s proxy, compares file-write/restart order and inspects
the import script. At :1480–1494 it independently checks that a modeled diagnostic masks the supplied proxy marker.
provisioning/e2e.rs:373–399 discards file contents and classifies the event by name. podman/tests.rs:608–631 checks the
serializer in isolation. Neither ties the freshly read map to the written file at the boot path, and the modeled script
does not establish the effective service environment on first launch (l8). Production podman.rs at
7ceb053a0c34908b8f081ec48d2f31fd1b711a1a:637–662 serializes and installs the supplied fresh map, so the file map is a
regression gap rather than a current wrong-map claim. podman.rs:689–765 reuses the existing container for start or
unpause and does not refresh proxy settings. The absence of a refresh on restart is intended under SPEC.md:5318–5320 at
b23bbba44fa7bcf0d111696ceb744c54042e6eff. Witness the file path and bytes, including a removed key and a distinct
present empty key. Bucket other. Confidence confirmed by existing source and assertion scope. Possible coverage none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `podman/l6`.
