# Inherited direct copies can damage a fixture pair, including on the new stop-failure publication path

Reviewed commit: c9660a5b670f8528cdd4ba9674b71270ffdc192b

## TLDR

A fixture copy failure can damage the previous screen and title pair and discard the staged replacement.

## Details

**other.** Source inspection.

Base 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e scripts/capture-agent-screens.py:720–741,784–795; reviewed
c9660a5b670f8528cdd4ba9674b71270ffdc192b scripts/capture-agent-screens.py:334–346,780–807,853–861; pinned
1a53d9d35889d7dde96e31320cc1681dd013737d crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs:120–149,176–186 and
docs/agent-screen-fixtures.md:25–31,39–43.

A failed or interrupted copy can leave a truncated fixture or a screen and title from different captures while default
cleanup removes the fully staged replacement. Direct per-file replacement and scratch cleanup were already present at
the exclusive range base. c9660a5b6 did not make publication non-atomic; it newly lets captures after an unconfirmed
stop reach the same copy loop. The Rust fixture loader pairs files by name with no generation check. CPython 3.12
shutil.py:258–283,475–477 confirms that copy2 opens the destination for a truncating write before it copies. This is an
inherited publication failure mode that now also affects newly retained stop-failure evidence. Keep a complete prior
pair and retain the staged replacement if the publication fails. The reviewer suggested other. Confirmed conditional
source behavior; no incident asserted.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`capture/capture-partial-fixture-publication`.
