# Malformed-row tests retain the old field layout and no longer isolate earlier validation

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The malformed-pane regression can pass even if earlier validation regresses, reducing confidence in the pane-fact
parser.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
behavior.

Source locations:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/tmux/control_codec.rs:1325-1329.

PANE_FACT_FORMAT now has seven fields (45-48), and parse_pane_facts_flagged requires the sixth field to be 0 or 1 and
the seventh to be a nonempty session name (170-176). However, parse_pane_facts_skips_every_malformed_row_shape retains
six-field rows for malformed pane id, window id, index and pane_dead (1325-1329). They all also have fh where the flag
now belongs and no session-name field. For example, allowing pane_dead=2 would not make the %5 row reach insertion: its
new flag is independently invalid. Give each earlier negative case a valid new-layout suffix (s 0 fh) so its intended
bad field is the sole rejection reason.

the patch adds a mandatory 0/1 tab flag between status and session name, but keeps old-layout malformed cases for pane
id, window id, window index and pane_dead. Each such row is therefore independently malformed at the new tab
flag/session fields.

the malformed-pane regression can pass even if earlier validation regresses, reducing confidence in the pane-fact
parser. Details/source: tip control_codec.rs:45-48 emits seven fields; :170-176 requires a sixth-field 0/1 marker flag
and a seventh nonempty session name. The malformed-pane/window/index/dead-value fixtures at :1325-1329 still contain six
fields, ending in s fh. Thus their flag is independently invalid and their session name absent. In particular, changing
:153-157 to accept pane_dead=2 would still leave the %5 row rejected at :170-176, so the final states.is_empty assertion
at :1333-1335 would not detect that regression. Give each earlier negative row a valid s 0 fh suffix, retaining its
intended single malformed field. No runtime mutation testing is needed for this static implication and none was
performed.

Possible coverage and caveats: none found after pinned-tip searches of all required coverage categories; Planned is
empty (:23-25); duplicate-pane-facts-can-unprotect-spare-claim.md in TRIAGE_OUTCOMES.md:18535-18554 specifies
duplicate-row uncertainty, not independent malformed-field rejection; plans/queue/tab-output-hangup.md:42-95 audits tab
death/cleanup, not these fixture suffixes. The current queue INDEX and matching-file search yielded no same
trigger/consequence/scope item; FILTER.md has no applicable test-oracle exclusion.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g057/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7987d070e91684f4c52df7695136cb87e66bd94f;
unchanged at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (tip lines 1312-1337 read).. This was a source-only
review; runtime occurrence and frequency were not measured.
