# Escaping the start remedy changes its paths

Reviewed commit: f7f2c18754db58761dd9ab6c170e63a5de802d87

## TLDR

Pasting the suggested supervisor-start command can start Farhelm against a different directory. If that path belongs to
another inactive installation using an older database, the mistaken startup can migrate it and prevent its original
program from starting. The conditional source chain is established; no such damage was reproduced.

## Details

Suggested bucket: conditional **highest**, for collateral installation damage; command/path corruption alone is
**other**. Confidence: **confirmed conditional source mechanism**. The second installation, path relationship and older
program/database pairing are deployment premises, not missing source behavior. Incidence is unmeasured. No physical
session-data loss, live-incumbent migration or remote-to-local execution chain is asserted.

`crates/farhelm-helm/src/manager.rs:4801–4817` shell-quotes configured program/state paths, then `4917–4966`
Debug-escapes the complete diagnostic. This doubles backslashes and alters apostrophe quoting. A single-quoted path
containing one literal backslash is presented with two; a shell preserves both. GUI peer rendering preserves those
printable bytes (`crates/farhelm-ui/src/hosts.rs:674–678`; `peer.rs:159–180,304–324`). Registration accepts both
spellings (`crates/farhelm-helm/src/store.rs:1733–1765`). The regression checks the helper before final normalization
(`manager.rs:5117–5130`).

Startup creates the selected state directory if absent (`crates/farhelm-supervisor/src/service/core.rs:5465–5492`), so
wrong-directory startup needs no planted alternative executable or pre-existing directory. Ownership protects a live
incumbent before database opening (`5510–5523,5556–5557`). An inactive installation can still be claimed and migrated.
That admission remains present at pinned tip `2ea2826caa0eec867c4e2bddc01badbac7bbe6c7`, `service/core.rs:5534–5571`.

The continuation establishes a concrete lasting consequence. At the reviewed commit,
`crates/farhelm-supervisor/src/store.rs:2273–2283` commits the migration from schema 30 to 31 before startup finishes.
The actual preceding program at `2bf117ef8d02e66b35cb9b9d204b94977fad639f` supports schema 30 (`store.rs:88`) and
refuses schema 31 (`2285–2290`, reached through `2832–2838`). Thus, if the corrupted state path names an inactive
installation with that pairing, exiting the mistaken process does not restore its ability to start. No alternative
executable is needed for this state-path consequence; executing another program would require one to exist at the
changed executable path.

Possible cover: at the extended pinned tip, deliberate same-account setup acceptance in `SPEC.md:3174–3187` excludes
bricking or similar lasting damage. The copied-command exception (`3479–3483`; `TRIAGE_OUTCOMES.md:13245–13277`)
concerns other configured fields. Ordinary one-way migration acceptance (`SPEC.md:3561–3584`) does not authorize
migrating the wrong installation. The rare-diagnostic and upgrade filters do not accept wrong-target durable changes or
an installation coming up broken. No exact coverage was found. Proposed drop: none. Preserve a correctly quoted command
separately from escaped diagnostic prose.

Provenance: original trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g094/f2`, independently checked by
`g143`. The same auditor continued in run `30f46c42-a021-475f-96a8-eac06930471b` as `legacy-g143`, resolving the source
blocker and completing separate empty discovery. The earlier ambiguous verdict is superseded by this conditional chain.
