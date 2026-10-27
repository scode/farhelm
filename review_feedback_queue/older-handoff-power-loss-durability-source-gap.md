# Older handoff retains an unresolved power-loss durability premise

Reviewed commit: 918eea3a7613d74fd8deb2886ced1d5c542626d3

## TLDR

The historical installer uses copy and rename without explicit flushes. Supported Mac utility and filesystem guarantees
establishing an unusable post-power-loss installation remain unverified.

## Details

`scripts/fixtures/install-v0.27.0.sh:432–451`, called at `1239–1269`, uses `cp`, `chmod` and `mv` without explicit
durability barriers. Published-source correspondence and the production handoff are established. Trailing-tip
`SPEC_impl.md:4640–4669` specifies flush ordering; `SPEC.md:3182–3191` does not exempt installations from power-loss
requirements. The modern replacement explicitly synchronizes bytes before rename.

Suggested bucket: `other`; established installation unavailability could justify `high`. Confidence: incomplete
platform/source lead. Missing are authoritative supported-macOS utility/filesystem semantics and an allowed persistence
outcome causing the failure. No separate work-loss route is established. Possible cover: no exact old-handoff durability
acceptance found; absence of explicit shell flushes alone does not prove utility behavior or APFS loss. Provenance: g017
discovery2. This is separate from the confirmed failure to repair an already damaged regular file. No crash, filesystem
or utility experiment ran.
