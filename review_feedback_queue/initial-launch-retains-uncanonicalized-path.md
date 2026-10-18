# Initial launch can resolve a different folder from its recorded directory

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

If a requested directory path resolves somewhere else while a session is being created, the new agent can start in a
different folder from the canonical directory Farhelm recorded. The path split is confirmed; an accidental production
trigger and harmful writes have not been demonstrated.

## Details

`crates/farhelm-supervisor/src/service/core.rs:7648–7672` canonicalizes the requested directory for stored identity but
retains the original spelling for launch. That spelling reaches tmux through `:9739–9740,13987–13991` and
`crates/farhelm-supervisor/src/tmux.rs:2846–2892`. A changed symlink component between canonicalization and tmux's path
resolution can therefore separate the actual launch directory from the recorded canonical directory. Directory admission
excludes Farhelm's own concurrent archive/delete operations; it does not make the two spellings identical.

Suggested bucket: provisionally **highest**, retaining the unresolved possibility of agent writes to unintended work.
Confidence: **confirmed spelling/canonical split; unverified accidental trigger and harm**. Ordinary file edits do not
change directory identity. Removal or rename alone does not prove execution in another directory. No remote authority
escape or data loss was observed.

Possible cover: SPEC.md:2807–2831, “Local authority and trust between hosts,” and the ledger's
`canonical-path-replacement-race.md:8588–8599` accept deliberately reshaped paths. That demonstrated deliberate case is
excluded from this finding. The retained remainder is accidental concurrent path replacement during initial creation,
which the independent reviewer did not find refuted. SPEC.md:2792–2799 deliberately allows changed symlink resolution
for terminal tabs; this is initial agent creation. Managed-path out-of-band manipulation acceptance does not establish
coverage for every ordinary existing-directory launch. No exact Planned, BUGS or queue match was found.

Proposed drop: the producer proposed dropping the unestablished accidental case; independent fresh-context disposition
review 2 said KEEP that narrow remainder. This does not turn an unverified accidental schedule into an observed bug.
Verification must establish such a schedule and inspect actual cwd before claiming damage. Launching the canonical
spelling would remove this extra spelling split, but would not by itself pin filesystem identity against later changes.

Provenance: `supervisor-create-1/f2`, latest `turn3.result.md`, and `drop-review-2/verdict.md` KEEP. This is separate
from the unusable-directory fallback finding: canonical spelling does not establish permission to enter the directory.
The producer's earlier HOME extension was withdrawn; later independent fallback evidence belongs to that other item.
