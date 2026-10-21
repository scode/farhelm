# Reset and search selection can restore another agent's custom flags

Reviewed commit: fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1

## TLDR

After Reset and choosing another agent through search, selecting Custom can restore the previous agent's permission
flags. Those words can be submitted after an explicit further selection; no harmful vendor interpretation was
demonstrated.

## Details

With remembered explicit YOLO, choose Claude/Custom and enter valid nonempty flags. Reset restores remembered YOLO and
clears the selected agent, but leaves the raw flags (`crates/farhelm-ui/src/list/create_form.rs:5888–5933`, especially
`5904–5924`). Search-select Codex: the agent-absent path manufactures `before.harness` from the destination (`1029`),
and Codex supports restored YOLO. Reconciliation therefore changes neither that harness nor permission and the clearing
guard (`1062`) does not run. Selecting Custom preserves the buffer (`7121–7132`, especially `7130–7131`); search-based
Custom does likewise (`1145–1154`). Earlier Claude words reappear under Codex.

Ordinary syntactic permission admission cannot tell stale words from intentionally entered Codex flags. This path is
distinct from chip-based switching, which clears even with no prior agent, and from effort-only model changes. Reconcile
buffer ownership at Reset/agent-absent transitions. No automatic launch or demonstrated approval bypass is asserted.

Suggested bucket: provisionally **highest**, for potentially unintended security-relevant permission words. Confidence:
**confirmed source-level transfer; browser behavior, a concrete dangerous compatible flag and harmful vendor effect
unverified**. A safe rejection or harmless interpretation would lower the consequence; that uncertainty is retained.
Proposed drop: none.

Possible cover: `SPEC.md:578–584` requires agent changes to clear Custom words; `617–618` restores remembered
permissions on Reset. `SPEC_impl.md:3160–3164` requires clearing after agent/permission edits, without this buffer
exception. Ledger `14324–14359` concerns retry normalization rather than this trigger. No exact Planned, BUGS, queue or
filter match was found. Provenance: `audit-launcher-permission-drafts/discovery-f1`, discovery pass 1 in the trailing
review through the reviewed commit. Prior direct-switch and effort-admission exclusions remain limited to their
inspected paths. No runtime reproduction was performed.

Original finding IDs: `audit-launcher-permission-drafts/discovery-f1`.
