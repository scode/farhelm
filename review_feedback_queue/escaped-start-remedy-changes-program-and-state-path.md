# Escaping the start remedy changes its paths

Reviewed commit: f7f2c18754db58761dd9ab6c170e63a5de802d87

## TLDR

Pasting the suggested supervisor-start command can start Farhelm against a different directory. Damage to another
inactive installation is conditional; no such installation or damage was established.

## Details

Suggested bucket: provisionally **highest** for unresolved collateral installation damage; confirmed command/path
corruption alone is **other**. Confidence: **confirmed source corruption and wrong-directory startup; severe harm
remains conditional on missing premises**. Runtime frequency was not measured, and no remote-to-local execution chain is
claimed.

`crates/farhelm-helm/src/manager.rs:4801–4817` shell-quotes configured program/state paths, then `4917–4966`
Debug-escapes the complete diagnostic. This doubles backslashes and alters apostrophe quoting. A single-quoted path
containing one literal backslash is presented with two; a shell preserves both. GUI peer rendering preserves those
printable bytes (`crates/farhelm-ui/src/hosts.rs:674–678`; `peer.rs:159–180,304–324`). Registration accepts these path
characters (`crates/farhelm-helm/src/store.rs:1733–1764`). The regression checks the helper before final normalization
(`manager.rs:5117–5130`).

Startup creates the selected state directory if absent (`crates/farhelm-supervisor/src/service/core.rs:5465–5492`), so
wrong-directory startup needs no planted alternative executable or existing directory. A live incumbent's database is
protected by ownership before opening (`5510–5523,5556–5557`). An inactive installation at the changed path can still be
claimed and migrated (`store.rs:2836–2845`; compatibility limits at `1494–1511`). Missing severe-impact premises:
another owned installation occupies that path, and migration or retention causes lasting installation/work harm.
Execution of another program additionally requires one to exist at the altered executable path.

Possible cover: the deliberate same-account setup acceptance at pinned `SPEC.md:3116–3137` covers deliberately planted
malicious alternate programs, but does not clearly accept ordinary path corruption and excludes bricking/corruption. The
copied-command exception (`SPEC.md:3429–3433`; `TRIAGE_OUTCOMES.md:13245–13277`) concerns other configured fields.
Independent auditor `g143` returned **ambiguous** and stopped the severe-impact causal investigation incomplete.
Proposed drop: none. Preserve a correctly quoted command separately from escaped diagnostic prose.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g094/f2`, independently checked by `g143`.
