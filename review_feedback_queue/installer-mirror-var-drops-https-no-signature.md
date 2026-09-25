# install.sh's mirror variable drops HTTPS with no signature check

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Someone who set the helm's mirror variable in their shell can, by re-running the installer, get tampered Farhelm
binaries fetched over unencrypted HTTP with no real warning.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F23 / SEC-MIRROR-VAR-NO-SIGNATURE`, tagged **definite**. Anchors and title: `scripts/install.sh:796-803`,
`scripts/install.sh:196-203`, `scripts/install.sh:898-902`, `scripts/install.sh:961-1012` — install.sh honours the
helm's documented mirror variable, dropping HTTPS pinning with no signature check

Normally every installer download goes through `curl_get` with `--proto '=https' --proto-redir '=https'`, so neither the
request nor any redirect can fall back to plain HTTP. When `FARHELM_RELEASE_BASE_URL` is set, the installer switches
`curl_get` into an override mode that drops both flags (L196–203), and `validate_release_base_url` accepts `http://` for
any host (L796–803). `SHA256SUMS` and the archives are then fetched from that server, following redirects (`-L`),
including an `https://` mirror's redirect to plain HTTP. The archives are checked only against that same server's
`SHA256SUMS` (L961–1012). The release's signature file `SHA256SUMS.minisig` is never checked. The `farhelm --version`
sanity check does not help, since it runs the downloaded binary, which can print anything. The only sign that anything
is different is one stderr line, `using FARHELM_RELEASE_BASE_URL=…`.

The comment at L898–901 calls the variable a deliberately undocumented test-only hook. But the same name is the
documented environment form of the helm's `--release-base-url` flag (`crates/farhelm-helm/src/lib.rs:301`, SPEC_impl.md
"## CLI", and `farhelm helm run --help`). There it is safe because the helm verifies the minisign signature with a key
built into the binary; the old README states "a mirror can serve the bytes but cannot alter them". An operator who
exported the variable for their helm therefore gets unauthenticated installs from their next `curl | sh`, on the machine
that holds the helm token and SSH access to the fleet. SPEC_impl.md accepts that "installing trusts GitHub over TLS"
only for the default channel; this path trusts neither GitHub nor TLS.

Suggested change: give the installer's test hook its own name (for example `FARHELM_INSTALL_TEST_BASE_URL`); allow
`http://` only for loopback and keep `--proto-redir '=https'` for `https://` mirrors; or embed the release public key
and verify `SHA256SUMS.minisig` and its version comment the way provisioning does. Either way, fix the L898–901 comment.

Restater note: exploiting this needs the variable to be present in the shell that runs the installer, plus either an
`http://` mirror, a mirror that redirects to HTTP, or a compromised mirror, with an attacker positioned to tamper. A
legitimate mirror for a different version than "latest" fails the `--version` check rather than installing silently.
