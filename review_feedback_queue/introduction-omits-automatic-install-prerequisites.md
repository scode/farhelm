# Introduction omits prerequisites for automatic host installation

Reviewed commit: 9f759eb6a62ee475e8a83563e0b6aa7c1e6c00a1

## TLDR

The new artwork promises automatic installation on any SSH-accessible host, although ordinary Linux hosts without a
usable systemd user manager require manual setup.

## Details

Suggested bucket: `other`. Confidence: confirmed. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed
drop: none.

website/scripts/render-svgs.mjs:206–210 adds 'Farhelm installs itself there for you' under the SSH-only prerequisite;
both generated SVGs:33–34 ship it. The host inspection explicitly returns Manual for unsupported architectures or an
unusable systemd user manager (crates/farhelm-helm/src/provisioning/backend.rs:2973–2991). SPEC.md, Topology (:119–126),
states these limits. Trigger: adding an SSH-accessible Linux host lacking the required user manager. Consequence: the
advertised automatic setup is unavailable and the user must install/start the supervisor manually. Qualify the
installation claim. The broader SSH-accessibility slogan predates this commit; the automatic-installation assurance is
new.

Possible cover: The detailed host guide supplies prerequisites, including later clarification in
c33fd86563711273c342b134777f646459f1ae4a, but that change does not correct the shared introductory artwork.

Current-main status: Persists at pinned main: render-svgs.mjs:206–210 retains the wording, backend.rs:2985–2999 retains
the manual fallback, and SPEC.md:119–126 retains the requirements. No fixing commit for the introductory claim
identified.

Review coverage: full commit diff and relevant code paths read. Runtime behavior and harmful reliance were not
reproduced; the note preserves the reviewer's stated unverified premises.
