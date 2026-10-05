# Updater release fixtures

A miniature release as get.farhelm.io serves it, for the desktop updater's install-step tests in
`crates/farhelm-ui/src/desktop/updater.rs`: a `SHA256SUMS` that lists an `install.sh` (and one stand-in payload, so the
file has more entries than the updater needs, as the real one does), its minisign signature with trusted comment
`farhelm v1.2.3`, and the `install.sh` it lists. The tests serve these bytes through the install step's fetch seam, so
the real verification (`farhelm_helm::verify_signed_sums` with this directory's key as the ring, then the installer's
SHA-256) runs on real signed bytes.

`variants/` holds what a test cannot make by changing served bytes alone, because the signature check would fail first
and report the wrong refusal:

- `wrong-comment/SHA256SUMS.minisig`: the same `SHA256SUMS`, correctly signed, with trusted comment `farhelm v1.2.2`.
- `no-installer/`: a `SHA256SUMS` without an `install.sh` line, correctly signed for `v1.2.3`.

A tampered `SHA256SUMS` (bad signature) and an `install.sh` that does not match its entry are made in the tests
themselves.

## The signing key

`test-key.pub` is a throwaway key generated for these fixtures. It has nothing to do with the release keys compiled into
Farhelm (`RELEASE_KEY_RING`), and its secret half was deleted right after signing: running the tests needs only the
public key. To regenerate, from the repository root, with minisign 0.12 on `PATH`:

```sh
F=crates/farhelm-ui/tests/fixtures/updater-release
W=$(mktemp -d)
printf '#!/bin/sh\necho "updater fixture installer"\n' >"$F/install.sh"
printf '#!/bin/sh\necho "updater fixture payload"\n' >"$W/farhelm-aarch64-apple-darwin.tar.gz"
(cd "$F" && sha256sum install.sh) >"$W/i.line"
(cd "$W" && sha256sum farhelm-aarch64-apple-darwin.tar.gz) >"$W/p.line"
cat "$W/p.line" "$W/i.line" | sort -k2 >"$F/SHA256SUMS"
mkdir -p "$F/variants/no-installer" "$F/variants/wrong-comment"
cp "$W/p.line" "$F/variants/no-installer/SHA256SUMS"
cp "$F/SHA256SUMS" "$F/variants/wrong-comment/SHA256SUMS"
(cd "$W" && minisign -G -W -p test-key.pub -s test-key.key)
minisign -S -t "farhelm v1.2.3" -s "$W/test-key.key" -m "$F/SHA256SUMS"
minisign -S -t "farhelm v1.2.3" -s "$W/test-key.key" -m "$F/variants/no-installer/SHA256SUMS"
minisign -S -t "farhelm v1.2.2" -s "$W/test-key.key" -m "$F/variants/wrong-comment/SHA256SUMS"
rm "$F/variants/wrong-comment/SHA256SUMS"
cp "$W/test-key.pub" "$F/test-key.pub"
rm -rf "$W"
grep -rl "$(id -un)" "$F"   # must find nothing
```
