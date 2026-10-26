# Canonical host replies change remembered folder defaults

Reviewed commit: 4beddcfddbf1cbaba87a0e2b2a790f9e4aa6e1de

## TLDR

Host canonical-path replies become remembered folder text, despite a product rule saying those replies must not choose
remembered defaults. The implementation specification explicitly carries canonical paths into history; spelling changes
are established, arbitrary path substitution is not.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-helm/src/workspace_trust.rs:88-90`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Later-turn synthesis complete. Reviewed commit: 4beddcfddbf1cbaba87a0e2b2a790f9e4aa6e1de; current coverage and
implementation checked only at pinned tip 463cac1528b5c5e259404975380d321a3fff0412. Verdict on g018/f1's complete
source-refutation exclusion: DISAGREE. Keep a narrow other-priority conditional contract record; proposed_drop: no.
TLDR: with managed workspace trust enabled, the folder remembered for later launches can be the target host's
canonical-path reply rather than the spelling the user submitted. Source: sessions.rs:1499-1503 normalizes mutable
req.cwd; workspace_trust.rs:76-90 replaces it with the returned path when managed; client.rs:2924-2943 returns the
peer's path and membership boolean without an input-equivalence check.
sessions.rs:1521-1538,1665-1668,1704-1708,1775-1790 carries it into explicit-selection, non-repository history.
store.rs:231-245 stores the path, :7395-7399 retains no repository provenance, :7682-7687,:7732-7736 returns the row.
The reviewed plan per-host-last-folder.md:25-31 selects this row. At pinned tip create_form.rs:1110-1115,1218-1221
implements that selection. No source premise for this data flow is missing; occurrence frequency was not measured and is
not a refutation. The reviewed commit is a planning patch, so this is an interaction with existing normalization and the
subsequently implemented default, not a claim that this patch introduced normalization. Contract: pinned
SPEC.md:3457-3462 requires GUI-request choices to shape future defaults/history, never host replies.
SPEC_impl.md:2406-2421 intentionally carries managed-trust canonical cwd through approval, dispatch and history to
prevent symlink retargeting; :3451-3453 separately describes canonical identity beside submitted display spelling, and
:3504-3515 derives the default from history cwd. Canonical spelling of the same directory is legitimate and must not be
classified as arbitrary destination substitution. An arbitrary/misbehaving host reply can traverse the same source path,
but whether the specific canonical-history acceptance authorizes it as a remembered GUI default remains unresolved;
keyword overlap cannot settle that. possible_cover: those SPEC_impl canonical-history paragraphs and
TRIAGE_OUTCOMES.md:1787-1795, which permits reply-only canonical facts while requiring submitted selections. The prior
ledger item concerns reply.launch permission/default poisoning and does not unambiguously cover mutable folder display
spelling. All six coverage sources were searched again at the pinned tip; no exact TODO Planned, BUGS or queue match,
and no matching unlanded-plan fix established. FILTER does not fully cover a durable/default-authority conflict. Tests
inspected, not run: pinned sessions_tests.rs:455-591 explicitly preserves submitted ~/ spelling beside
accepted/canonical path; :4947-5176 intentionally uses a different managed canonical dispatch path but does not assert
resulting history. Reviewed store.rs:9325-9398 keeps alias spelling separate from accepted canonical identity. These
support the contract distinction and do not prove an enabled-trust history invariant. Confidence: confirmed mechanism,
conditional contract defect. Suggested bucket: other. The same target already owns its processes and files; provenance
mismatch alone establishes no security impact. No cross-host operation, credential exfiltration, or user-work loss has
been shown. Such a stronger bucket needs an additional source-established harmful route (or separate established
materially harmful UX), not merely an occurrence measurement. Preserve earlier possibly-highest wording and withdrawals
for independent audit, but do not treat them as current severity. Narrow exclusions remain: AGREE for
Agent-origin/no-explicit-selection (sessions.rs:1665-1669; store.rs:205-211), AGREE for fresh-checkout rows (trusted
repository intent and plan filter), AGREE for request_fingerprint reply override as a GUI-history route (GUI
constructors :1538,:2121,:3686 use None; agent_requests.rs:2526-2543 has Agent origin). None excludes workspace-trust
normalization. Parent reports independent confirmation of the current flow and is asking g018 to correct its refutation.
Discovery remains a separate actual turn.

Provenance: `a010/f1`, `g018/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
