# Newline parsing changes the initial login-shell directory

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A newline-bearing folder can be transformed before the initial login shell starts; a harmful supported startup operation
remains unestablished.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Checksummed pinned tmux 3.7c parsing source confirms the initial-directory transformation. The later shim independently
restores verified cwd and PWD before the final agent, refuting that wrong-agent-directory claim. No shipped/evidenced
startup action establishing the stronger consequence was found; arbitrary user RC code is not proof. The report-only
overstatement is separately excluded, not used to erase this narrow product uncertainty.

Source locations: `plans/reports/tab-output-hangup.report.md:182-185; crates/farhelm-supervisor/src/tmux.rs:3492-3496`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis pass complete for reviewed commit d6645f771bee8ed03a66486ef8755834937d759d; original report provenance
5812dd5d7210fb2874f6164169a097daae14404c. No new verified user-facing defect. Primary dependency source is now checked,
not unavailable: downloaded tmux-3.7c.tar.gz from the upstream GitHub release, SHA-256
7c60cae9a0e25288e2e24750aafc9e8800fc7fd4555e447e1b29ee4201cfb3bf, matching .github/release/source-pins.env:3-4. The
release build uses the generated parser (scripts/build-private-tmux.sh:150-160), whose cmd-parse.c:1417-1435 strips
spaces/tabs after literal newline even in single quotes. The grammar agrees at cmd-parse.y:1680-1698.
cmd-if-shell.c:86-101 calls args_make_commands_now; arguments.c:740-755,774-792,823-841 reaches cmd_parse_from_string,
establishing the actual nested parser corridor. spawn.c:229-241,381-387,428-431,463-466 format-expands the rewritten
cwd, falls back to HOME or / if chdir fails, and execs the supplied shell argv in that cwd. The report's bare newline-#
case is overbroad: cmd-parse.c:1427-1430 preserves #{, while tmux_start_directory at tmux.rs:2298 encodes path # as
#{a:35}. The possible source-only residual is a legal folder containing newline plus spaces/tabs causing initial
login-shell cwd and PWD to differ before the helper restores the final agent cwd. Final-agent
wrong-folder/user-work-loss route is refuted by the pre-existing exact cwd/PWD fence (core.rs:15232-15236,15359-15370;
launch.rs:1157-1162,1202,1240-1252). Parent reports independent agreement a026 for global g006/f2, the same final-agent
route; proposed exclusion is therefore available to parent synthesis. Report-only factual inaccuracy is other and
proposed_drop is covered by TRIAGE_OUTCOMES.md:20479-20495,20526-20538 operational-artifact exclusion. That exclusion
does not cover a new executable issue. Actual producer investigation: launch.rs:651-716 builds only the user's
interactive login shell followed by Farhelm's helper; 999-1051 writes a spec-derived waypoint and execs the supplied
wrapper; session_hold.rs:68-235 uses spec-derived marks and its own keeper, with no cwd-derived work selected there.
Pinned git grep across crates/scripts/packaging/.github found no direnv/autoenv integration, PROMPT_COMMAND hook, or
Farhelm-installed cwd-dependent rc-file action; matches for startup files were documentation and private test fixtures,
including restart_with_resume.rs:1785-1806 which only exports a constant. SPEC.md:2451-2465 intentionally executes user
rc files, and 1531-1535 explicitly restores cwd afterward. Exact missing premise for higher impact: an actual ordinary
shipped or evidenced supported startup operation that acts harmfully on the rewritten initial folder or changes lasting
agent configuration because of it. No such action, security crossing, wrong target mutation or owned-work loss is
established. The accessible pinned product and dependency sources establish the encoding and restoration boundaries but
cannot establish arbitrary user startup-file contents or their consequences. Resolving evidence would be source for the
actual startup operation and a complete causal path to harm; no runtime or vendor turns were performed. Suggested bucket
other for the established narrow source concern; highest requires the missing consequence route. Confidence: confirmed
parser transformation and final-agent fence; suspected environment/startup consequence unverified after investigation.
possible_cover: explicit post-startup cwd restoration may delimit the initial-cwd contract, but does not explicitly
accept transformed startup environment; no exact product acceptance or queued duplicate found. Keep the residual open
for parent synthesis; do not conflate it with the covered report statement.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g172/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
