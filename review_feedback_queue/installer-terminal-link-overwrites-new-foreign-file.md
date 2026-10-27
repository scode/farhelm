# Terminal-link publication can overwrite a new foreign file

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

Installation can overwrite a file another tool places at Farhelm's public Terminal-command location after the installer
checked or preserved its earlier contents.

## Details

In `crates/farhelm-supervisor/src/installation/install.rs:1038–1077`, the installer inspects the existing public entry
and preserves foreign contents. Publication later uses an overwriting rename at `1093–1099`, without binding the checked
identity or preserving a new occupant. The legacy desktop-copy branch similarly checks a digest before removing the
pathname (`1049–1053`). Another same-account writer need not participate in Farhelm's lock. A valid concurrent arrival
can therefore be lost despite the foreign-file preservation guarantee. This is separate from an already occupied
fallback backup name.

Suggested bucket: `highest`. Confidence: confirmed conditional source mechanism, independently checked by `a004`;
adverse scheduling and loss frequency are unmeasured. No runtime reproduction ran. This does not allege cross-account
access or that any named external tool normally writes this location.

Possible cover: trailing-tip `SPEC.md:443–454,3320–3333` explicitly preserves foreign public entries even while
assigning recognized bundle contents and reserved temporary names to Farhelm. The occupied-backup acceptance and
interrupted foreign-app cleanup exception have different triggers and locations. No matching whole-finding exclusion was
established.

The historical handoff supplies another instance at reviewed commit `918eea3a7613d74fd8deb2886ced1d5c542626d3`.
Published v0.27.0 bytes match `scripts/fixtures/install-v0.27.0.sh`; its inspection and preservation at `1316–1329`
precedes overwriting public-name publication at `1346–1350`. Independent a002 synthesis resolved the served-source
premise and identified this as the same defect, not another queue item.

Provenance: `g011/f2`, `g015/f6`, `a004/f1`, `a002/terminal-file-lead`. A remedy must address the public-name
transition, rather than relying solely on Farhelm's installation lock. The separate crash-persistence question does not
weaken this live-filesystem overwrite route.
