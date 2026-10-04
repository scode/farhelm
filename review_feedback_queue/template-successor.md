# Template installation identity ignored at create dispatch

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Template installation identity ignored at create dispatch.

## Rebase context

At rebase onto `eb71c32e0f14f4569938e7da293e366aa07eae8a`, #1627 (`bda681e615fe283c8c93b52a5b614d91c86d313b`) removes
saved create resolutions and requires explicit `--host` for keyed creates. The saved-resolution retry variant and the
remedy to retain a keyed resolution below no longer apply. The finding remains for an unkeyed create whose template
selects an installation: resolution matches that identity to a registry row
(`crates/farhelm-helm/src/agent_requests.rs:1320–1326`), but dispatch later takes that row's current connection without
comparing the template identity (`:1514`). The approval's own incarnation check (`:1433–1452`) does not establish this
comparison. Verify the template-selected identity at dispatch; explicit host overrides remain excluded. The original
reviewed revision and reviewer prose below are retained as provenance.

## Details

`F7 / COR-TEMPLATE-SUCCESSOR` — **definite** — `crates/farhelm-helm/src/agent_requests.rs:1177` — Template installation
identity ignored at create dispatch

A template identifies its target host by the installation’s recorded identity, so replacing the machine behind a
registered host should make that template inapplicable. During agent-create resolution, the helm checks the template
identity and finds the corresponding registry row. It saves both the resolved launch and that row’s durable ID.

Dispatch later obtains whichever supervisor connection currently serves that row. It does not compare that connection’s
installation identity with the identity frozen from the template. If the row is retargeted or a replacement installation
is adopted between resolution and dispatch—or before a retry reuses the saved resolution—the create can run on the
replacement machine. The bookkeeping checks ensure that subsequent writes describe the connection actually used; they do
not ensure that it is the installation the template selected.

Before dispatch, compare the template’s frozen installation identity with the connection claim obtained alongside the
client. Refuse a mismatch and retain the keyed resolution so that a retry cannot silently resolve against the successor.
Requests carrying an explicit `--host` override are excluded: that override intentionally replaces the template’s host
choice.

Suggested bucket: **highest**. No possible cover was identified. The missing identity gate is confirmed by source
inspection.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `helm_lifecycle p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Explicit --host override excluded.

## Filed reviewer metadata

- `helm_lifecycle p1`: confidence as filed: **definite, confirmed** by the stored resolution, dispatch path, and
  installation-binding contract. Suggested bucket as filed: **highest** — execution on the wrong machine.
