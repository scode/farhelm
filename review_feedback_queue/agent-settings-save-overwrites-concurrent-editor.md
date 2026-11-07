# Agent settings save overwrites concurrent editor

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313

## TLDR

Saving agent settings can overwrite a benign concurrent edit, including a first settings file created while Farhelm was
preparing its save.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location:
`crates/farhelm-helm/src/provisioning/tensorlake_backend.rs:91-145 at ca28b604b876770b1ab757783416f83ad1709313`.

Source assessment (read with the later corrections below) (review record g053/f1, event 13):

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313. TLDR: Running setup again on an existing Tensorlake host can
silently undo an unrelated Claude or Codex configuration edit the user saved while setup was running. Suggested bucket:
highest, because the demonstrated result loses a persisted user configuration edit. Confidence: confirmed source
interleaving; no runtime experiment. Complete benign interleaving: existing ~/.claude/settings.json contains the 24
ASCII bytes {"env":{"EDITOR":"vim"}}. A normal rerun uses a recipe with a Claude endpoint. (1) Initial wc -c returns 24.
(2) The one dd reads those old 24 bytes. (3) Before the second wc, a normal editor saves and persists
{"env":{"EDITOR":"zed"}} at the same regular-file path, using an ordinary atomic save if desired. Both are valid JSON
with an object env and exactly equal length. (4) The final wc returns 24, so the old bytes pass every read check. (5)
agent_endpoints::claude parses the old document, inserts ANTHROPIC_BASE_URL and ANTHROPIC_AUTH_TOKEN, retains
EDITOR=vim, and emits valid merged JSON M. (6) install_source's destination metadata describes the user's new document
B, but B does not equal M (M has the endpoint keys), so its new-content equality optimization does not skip
installation. (7) M is uploaded to a separate temporary; B's ordinary JSON first line is not the managed-unit marker.
The final command verifies the TEMPORARY against M's hash, chmods it and executes mv -f temporary destination. B is
replaced, silently reverting the already-persisted EDITOR=zed edit. No malicious writer, malformed document, symlink
manipulation, second helm or second GUI is needed. A single ordinary user edit and one setup rerun suffice. A file over
32 KiB also admits mixed generations across dd calls, but is not needed for the established loss. Exact path:
provisioning/tensorlake_add.rs:173-187 accepts an existing TensorlakeHost, without a completed-setup exclusion; :201-208
loads its recipe while retaining host size; :516-520 adopts the existing sandbox ID; :438-449 prepares it and configures
endpoints whenever set; :619-637 reads, merges and installs the complete settings file. backend.rs:2118-2123 routes
read_small_file to read_agent_file; :655-675 and :735-766 launch a distinct remote sh per script.
tensorlake_backend.rs:106-145 has only initial/final length validation, no coherent snapshot or old-generation token.
agent_endpoints.rs:11-32 changes only the two owned env keys in the supplied document. backend.rs:2568-2595 stages the
new merged bytes and their hash; :974-990 compares the destination only to that NEW hash for idempotence. :1054-1094
uploads, checks only a managed-unit first-line marker on the destination, hashes the new temporary and performs mv -f.
ssh_put at :1287-1359 and upload_script at :1802-1811 stream to that temporary, not a locked snapshot of the
destination. The managed-unit marker check is an ownership check, not a settings-generation comparison. Neither digest
check compares the live destination to the settings read earlier. SPEC boundary check: SPEC.md at this commit, Topology,
setup paragraph :287-295, says 'Unrelated settings are preserved'. SPEC_impl.md, Adding Tensorlake hosts :5777-5785,
says 'only ANTHROPIC_BASE_URL and ANTHROPIC_AUTH_TOKEN change' and 'Only Add, including its rerun, writes endpoint
settings; Update does not.' It grants no stale-write exception. SPEC.md, Local authority and trust between hosts :3974
onward, excludes 'deliberate interference' by same-account processes, but also says Farhelm defends against 'mistakes a
user plausibly makes in ordinary use of Farhelm or their machine.' The listed deliberate filesystem examples require
special interference (imitation records, specially planted symlinks, exhausting preservation names); saving an ordinary
settings change is not such an arrangement and the overwrite does not follow from its intended meaning. SPEC.md, One GUI
at a time :4123 onward, accepts best-effort races among several GUIs and specifically accepts launch-template/number-key
whole-draft overwrites, not external agent-settings edits during host setup. Concepts :60-66 narrows the template
exception explicitly to launch templates. Those clauses do not refute this benign persisted-configuration-loss route,
and no threat-model escape is inferred from same-account authority alone. Temporal qualification: remains through pinned
tip 8a8c9579e04299410652f0e4681975a7198dfc7b. git log for backend.rs, tensorlake_backend.rs, agent_endpoints.rs and
tensorlake_add.rs from introduction to tip lists only c340166d754d3edb3de7fd538c6408948d3536c6; its tensorlake_add
change is unrelated tunnel-confirmation wording. The shared backend/read/merge code is unchanged. A targeted
SPEC/SPEC_impl diff search found no change to the cited authority, concurrency, preservation or read clauses through
that tip. Fix direction: coherent remote snapshot plus prevention or visible refusal of overwriting an intervening
benign edit; length checks and new-temporary hashes cannot establish that. proposed_drop: false. possible_cover:
unknown; the parent owns coverage checking and will send f1 to add/config merge owners. Caveats: requires a saved
endpoint and a save during the read-to-install interval; this is source evidence, not measured frequency or dynamic
reproduction. A guarantee against a malicious same-account adversary is a separate, excluded claim and is not part of
f1. Revision reason: completed rerun/adoption, uploader/promotion, exact accepted-scope and later-history checks;
replaced the illustrative unrelated theme key with a concrete unrelated environment setting and retained highest.

Additional source evidence (g053/d12, event 20):

Discovery-3 adds a concrete boundary variant to known g053:f1, not a novel root. read_agent_file's initial script exits
75 only when neither a file nor symlink exists, then :112-114 immediately returns None without a second
existence/generation check. configure_agents :621-622 merges None as a fresh document, awaits directory preparation,
then :635-636 installs the whole new document. If a benign editor saves and persists its first settings file after that
missing read and before installation, the shared installer treats its nonmatching destination hash as a reason to
install, checks only the unit marker, and mv -f replaces it. That persisted user configuration can be lost without an
equal-length edit or any multi-block read. This is the same absent old-generation/promotion precondition already owned
by f1, now for expected absence instead of an existing snapshot; not a second endpoint/root defect and not the
serializer output-limit issue g048:e1. Suggested consequence remains highest as for f1; source-confirmed conditional on
that ordinary save timing, no runtime or frequency claim. proposed_drop false; possible_cover exact g053:f1. Parent can
consolidate this supporting route into f1 if useful. No malicious same-account interference, vendor/project-status
premise, or separate highest exclusion is introduced.

Recording coverage: Pending credential reconciliation serializes Farhelm's own writers, not a benign external editor
writing persisted agent settings. No exact cause removal.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g053/f1`; supporting records
`g048/i1`, `g053/l5`, `g053/d12`, `g071/c8`. The retained private log preserves earlier wording and its corrections.
This record does not claim runtime reproduction.
