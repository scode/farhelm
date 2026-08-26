# Release download fixtures

A miniature GitHub release: the six published asset names, a `SHA256SUMS` over them, and a real minisign signature. The
tests in `provisioning/release_payloads.rs` serve this directory from a loopback `axum` server and drive
`ReleasePayloadSource` against it, so the whole verification chain — signature, trusted-comment version binding,
per-file SHA-256, single-member extraction — runs on real bytes rather than on stubs.

The "binaries" are two-line shell scripts. Their content follows one rule, and the tests reconstruct the expected bytes
from it rather than embedding a copy:

```
#!/bin/sh
echo "farhelm fixture: <package> <target>"
```

where `<package>` is `farhelm`, `farhelm-desktop`, or `tmux`. Change that rule and the tests fail loudly rather than
silently comparing nothing.

## The version is inside the signature

The trusted comment is covered by minisign's global signature, and it is the ONLY place the release version is
authenticated: the version in the URL and in the cache directory name is attacker-chosen, so without this binding a
valid older `SHA256SUMS` could be replayed at a newer version's URL and downgrade every host a helm provisions.

The comment is `farhelm` followed by the release TAG, and a tag is `vX.Y.Z` — it already carries the `v`, so the signing
flag is `-t "farhelm $TAG"` and never `-t "farhelm v$TAG"`, which would render `farhelm vv0.0.3`. The Step 5 `sign-sums`
job follows the same rule; a release signed without `-t`, or with the doubled `v`, is refused by every helm.

**Fixtures signed for the current version are pinned to workspace version `0.0.3`, permanently.** They are never
re-signed when the workspace version bumps, and that is the point: the first real release tag bumped the workspace
version away from `0.0.3` and broke every test in this module in one go, because at the time `ReleasePayloadSource` read
`env!("CARGO_PKG_VERSION")` for the version it expected, and that constant moves with every release while these
committed signatures cannot. `ReleasePayloadSource` now takes the version it expects as a constructor argument instead
of reading that constant itself — production passes `CARGO_PKG_VERSION` (one call site, in `payloads.rs`), and the tests
here pass `FIXTURE_VERSION` (`"0.0.3"`, defined beside this fixture's other helpers), which never changes. Two tests
hold the two ends of that contract together: `production_wiring_binds_the_cache_to_the_crate_version` (in
`provisioning.rs`) is the oracle that production still passes the real crate version, and
`signing_and_verification_agree_on_the_tag_convention` (in `release_payloads.rs`) is the oracle that the trusted-comment
convention itself — `farhelm v$TAG`, never `farhelm vv$TAG` — still matches what `sign-sums` produces.

**`variants/other-version/` is the exception and must stay signed for a different version** (`farhelm v0.0.2`). It is a
correctly signed manifest for the wrong release — the replay condition itself — so re-signing it for the current version
would silently delete the test rather than update it.

## The signing key

`test-key.pub` is a throwaway key generated for these fixtures and has nothing to do with `MINISIGN_PUBKEY`, the key
compiled into shipped binaries. The matching secret key is deliberately not committed: running the tests needs only the
public key, and a signing key has no business living in a public repository even when it signs nothing real.

That is a hygiene property, not a security one. Nothing here resists a determined edit — anyone can generate their own
throwaway key, alter an asset and its manifest, re-sign, and replace `test-key.pub`. What protects these fixtures is the
same thing that protects the rest of the tree: review of the change that touches them. What they verify is that
signature and checksum handling is wired up correctly, not that the repository is tamper-proof.

Complete regeneration therefore creates a FRESH key pair and rewrites `test-key.pub` and every signature together — they
are one set, and a partial regeneration leaves signatures that no longer verify.

Tests read the key line (the second line) out of `test-key.pub` and inject it into `ReleasePayloadSource::new`, so
regenerating the key pair below needs no code change.

## Regenerating

Everything here is generated together, from an empty directory, with the key created fresh. Run this from a scratch
directory, not from a checkout, and delete `test-key.key` when it finishes.

The `tar` flags are not optional. A plain `tar czf` records the generating account's user and group names in every
header, and these archives are committed binary data in a repository that must carry no local-environment detail. They
are spelled out at each call site rather than collected in a variable on purpose: `--mtime` takes an argument containing
a space, so an unquoted `$TARFLAGS` expansion would hand `00:00:00Z` to tar as a separate operand and the archive step
would fail — quietly, if the script does not stop on error.

```sh
set -eu

FIXTURES=<path to this directory>
VERSION=0.0.3   # FIXTURE_VERSION, permanently — NOT the workspace version
TAG="v$VERSION" # a release tag carries the leading v; the comment appends nothing
FIXTURE_MTIME='2026-01-01 00:00:00Z'

# 1. The four archives, at dist's <package>-<target>/<binary> nesting.
for spec in \
  "farhelm x86_64-unknown-linux-musl farhelm" \
  "farhelm aarch64-unknown-linux-musl farhelm" \
  "farhelm aarch64-apple-darwin farhelm" \
  "farhelm-desktop aarch64-apple-darwin farhelm-desktop"; do
  set -- $spec
  mkdir -p "$1-$2"
  printf '#!/bin/sh\necho "farhelm fixture: %s %s"\n' "$1" "$2" >"$1-$2/$3"
  chmod 755 "$1-$2/$3"
  tar --owner=0 --group=0 --numeric-owner --mtime="$FIXTURE_MTIME" --sort=name \
    -czf "$FIXTURES/$1-$2.tar.gz" "$1-$2/$3"
done

# 2. The two tmux builds, which ship unarchived (D5).
for target in x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
  printf '#!/bin/sh\necho "farhelm fixture: tmux %s"\n' "$target" >"$FIXTURES/tmux-$target"
  chmod 755 "$FIXTURES/tmux-$target"
done

# 3. SHA256SUMS over all six, sorted by name.
(cd "$FIXTURES" && sha256sum \
  farhelm-aarch64-apple-darwin.tar.gz \
  farhelm-aarch64-unknown-linux-musl.tar.gz \
  farhelm-desktop-aarch64-apple-darwin.tar.gz \
  farhelm-x86_64-unknown-linux-musl.tar.gz \
  tmux-aarch64-unknown-linux-musl \
  tmux-x86_64-unknown-linux-musl >SHA256SUMS)

# 4. The variants (see below).
mkdir -p "$FIXTURES/variants/without-tmux" "$FIXTURES/variants/two-member" \
  "$FIXTURES/variants/other-version" "$FIXTURES/variants/legacy-signature"
grep -v ' tmux-aarch64-unknown-linux-musl$' "$FIXTURES/SHA256SUMS" \
  >"$FIXTURES/variants/without-tmux/SHA256SUMS"
cp "$FIXTURES/SHA256SUMS" "$FIXTURES/variants/other-version/SHA256SUMS"
mkdir -p two-member/a two-member/b
printf '#!/bin/sh\necho "farhelm fixture: two-member a"\n' >two-member/a/farhelm
printf '#!/bin/sh\necho "farhelm fixture: two-member b"\n' >two-member/b/farhelm
tar --owner=0 --group=0 --numeric-owner --mtime="$FIXTURE_MTIME" --sort=name \
  -czf "$FIXTURES/variants/two-member/farhelm-x86_64-unknown-linux-musl.tar.gz" \
  -C two-member a/farhelm b/farhelm
two_member_sha=$(sha256sum "$FIXTURES/variants/two-member/farhelm-x86_64-unknown-linux-musl.tar.gz" | cut -d' ' -f1)
sed "s|^[0-9a-f]\{64\}\(  farhelm-x86_64-unknown-linux-musl\.tar\.gz\)$|$two_member_sha\1|" \
  "$FIXTURES/SHA256SUMS" >"$FIXTURES/variants/two-member/SHA256SUMS"

# 5. One fresh key, five signatures, then destroy the secret key. Note the
#    deliberately different tag on the other-version variant.
minisign -G -W -p test-key.pub -s test-key.key
for sums in \
  "$FIXTURES/SHA256SUMS" \
  "$FIXTURES/variants/without-tmux/SHA256SUMS" \
  "$FIXTURES/variants/two-member/SHA256SUMS"; do
  minisign -S -t "farhelm $TAG" -s test-key.key -m "$sums"
done
minisign -S -t "farhelm v0.0.2" -s test-key.key -m "$FIXTURES/variants/other-version/SHA256SUMS"
minisign -S -l -t "farhelm $TAG" -s test-key.key -m "$FIXTURES/SHA256SUMS" \
  -x "$FIXTURES/variants/legacy-signature/SHA256SUMS.minisig"
cp test-key.pub "$FIXTURES/test-key.pub"
rm -f test-key.key

# 6. Prove the headers carry no account name before committing.
tar -tvzf "$FIXTURES/farhelm-x86_64-unknown-linux-musl.tar.gz"   # must show 0/0
grep -rl "$(id -un)" "$FIXTURES"                                 # must find nothing
```

`the_committed_archives_record_no_account_identity` asserts step 6's first check on every committed archive, so a
regeneration that forgets the ownership flags fails the suite rather than shipping a name.

`-W` writes the secret key unencrypted, matching D3: the minisign CLI has no non-interactive password path, so CI's own
signing key is unencrypted too and these fixtures reproduce that shape.

`minisign -S` prehashes by default, which is what `minisign_verify`'s `verify(.., allow_legacy = false)` requires. The
`legacy-signature` variant is what proves that refusal is real rather than assumed.

## variants/

Four refusals cannot be reached by overriding served bytes alone, because an earlier check fires first and reports a
different (correct) error. Each therefore needs its own separately signed artifact, which the fixture server substitutes
for the real one:

- `without-tmux/` — the same file with `tmux-aarch64-unknown-linux-musl` removed, for "SHA256SUMS has no entry for
  {asset}".
- `two-member/` — an archive carrying two members named `farhelm`, plus a `SHA256SUMS` listing that archive's hash, for
  "{asset} contains 2 members named farhelm; expected exactly one".
- `other-version/` — the real `SHA256SUMS`, byte for byte, signed with `-t "farhelm v0.0.2"`. This is the replay attack:
  a perfectly valid older release manifest served at this version's URL. Only the trusted comment tells them apart.
- `legacy-signature/` — the real `SHA256SUMS` signed with minisign's legacy (`-l`, non-prehashed) format, for the
  refusal `allow_legacy = false` exists to produce.

All are signed with the same throwaway key, so they exercise real verification rather than bypassing it.
