# A devbuild's Update pushes a tmux to remote hosts that was checked only against an unsigned checksum list

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

When the maintainer updates a remote host from a dev build, the tmux program installed there was downloaded from the
release site and checked only against a checksum list from the same site over HTTPS. It is not checked against the
release signing keys that the released app checks for the same files. Anyone who can serve content as get.farhelm.io (a
compromised site or hosting account) could get their own tmux run on every host the maintainer updates from a dev build,
while the released app would refuse that tampered release.

## Details

Found by the 2026-10-22 prioritized persistent review, groups `215-maint-scripts-3of9` (finding f3) and
`216-maint-scripts-4of9` (finding f2), merged here as the same defect.

Mechanism:

- `scripts/devbuild-mac.sh:561-605` reads `release_tag` from `https://get.farhelm.io/latest`, then downloads
  `SHA256SUMS` and `tmux-<arch>-unknown-linux-musl` from `https://get.farhelm.io/<tag>/`, and accepts the binary when
  its sha256 matches the `SHA256SUMS` line. `SHA256SUMS.minisig`, the signature in SPEC_impl.md's "Verification chain
  (D3)", is neither fetched nor checked.
- The binary is cached under `target/devbuild/tmux/<version>/` and reused by later builds without rechecking (`:561`),
  then copied into the payload directory (`:603-604`). The dev helm treats that directory as operator-trusted and does
  not verify it (SPEC_impl.md, the provisioning payload paragraph: "files in an explicitly selected local directory are
  treated as operator-trusted and are not verified, on ANY build, developer or release ...
  (`scripts/devbuild-mac.sh`)").
- The header (`:38-48`) says this is deliberate: "fetched once and checked against its SHA256SUMS over TLS (the same
  trust a first `curl | sh` install has, not the signature)".

Scenario: get.farhelm.io serves a modified tmux with a matching `SHA256SUMS` (an origin compromise). The maintainer runs
a devbuild for the first time with that tmux version and presses Update on a remote host. The tampered tmux then runs
every session on that host.

How to verify: read `devbuild-mac.sh:561-605` and note that no `.minisig` is fetched and nothing calls a signature
verifier.

Fix sketch: fetch `SHA256SUMS.minisig` and verify it with the release public key before trusting `SHA256SUMS`, using
minisign or the helm's own verifier (for example a small `farhelm` subcommand in the native CLI the script has just
built).

Suggested bucket: 215 f3 said high at most; 216 f2 said highest by consequence class (code execution on remote hosts),
noting that the trigger needs a compromise of get.farhelm.io under which a fresh install is equally exposed. Recorded
under high: the harm comes through maintainer tooling and needs a compromise of the release site. Triage settles the
bucket.

Confidence: confirmed by code reading, and stated in the script's header.

Reviewers' proposed drop, kept for triage: both reviewers proposed dropping it as covered. SPEC.md "Security" accepts
the same trust for a fresh install: "the app then says that it can no longer verify its updates and must be reinstalled
with the installer, which, like any fresh install, trusts get.farhelm.io over TLS alone." SPEC_impl.md makes the payload
directory operator-trusted (quoted above). And `review_feedback_queue/AGENTS.md`, "Priority buckets", keeps harm
confined to maintainer tooling out of highest unless it is catastrophic. Recording did not treat these as a full cover.
SPEC.md (around line 408) gives the installer's reason for TLS-only trust: "a fresh machine has no pinned key or
verifier to check it with". That reason does not hold for a devbuild, which runs from a checkout that carries the
release keys. The operator-trusted rule covers the helm not verifying the directory, not how the script fills it. 215 f3
named the same gap: the devbuild chooses TLS-only trust for something it pushes to remote hosts, when the release
signature is available.

Possible cover: SPEC.md "Security" and the installer paragraph (installer trusts get.farhelm.io over TLS alone);
SPEC_impl.md provisioning payload paragraph (operator-trusted `FARHELM_HELM_PAYLOAD_DIR`).
