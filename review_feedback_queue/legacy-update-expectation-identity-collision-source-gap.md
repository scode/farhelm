# Legacy installation expectations retain an identity-collision source gap

Reviewed commit: c4faa8adb0ad5ac009de2bc79b7efdce7905a4a1

## TLDR

Two legacy ownership records can have indistinguishable expected identities. A complete supported updater/replacement
history producing harm through that collision remains unestablished.

## Details

`crates/farhelm-supervisor/src/installation/mod.rs:490–493` represents a two-field legacy record with no installation ID
and generation zero. The expectation comparison at `715–721` cannot distinguish two such records carrying the same
Installed version. The modern differing-tuple refusal remains valid; it does not prove distinctness for identical legacy
tuples.

Suggested bucket: `other`. Confidence: source-established equality with missing current-updater, replacement and
consequential-history premises. No security, corruption or user-work-loss route is established. Possible cover: no exact
acceptance of a harmful identical-tuple history was found. Required legacy compatibility and intentional downgrades
alone do not settle that consequence. Provenance: `a005/f1`'s retained legacy-tuple remainder, distinct from its later
historical-installer route that ignores expectations entirely. No runtime experiment ran.
