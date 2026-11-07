# Qualify version agreement to launch time

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The newly documented agreement between a program's terminal-version environment and a terminal query needs a lifetime
qualification. A surviving program keeps its launch environment after a Farhelm update, while a new query after adoption
receives the current emulator's version. The established consequence is documentation ambiguity only; no application
failure is assumed.

## Details

Source coordinates from the original records:
`crates/farhelm-supervisor/src/service/core/host_launch.rs:85-89,1122-1134; SPEC.md:2421`;
`SPEC.md:2420-2424; crates/farhelm-terminal-emulator/src/lib.rs:124-127; crates/farhelm-terminal-emulator/src/queries.rs:405-408`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Introduced in assigned 160592cbf07281ccf145890dca17c6a9989cf40a SPEC.md:2420-2424 and SPEC_impl.md:2151-2159
(tip:2181-2189). The surrounding lifetime qualification is real and must be retained: tip SPEC_impl.md:2188-2189
explicitly says an earlier host 'keeps that environment until its agent is restarted'. Thus the change correctly leaves
existing children alone and does not promise that updating the supervisor rewrites their environment. However,
SPEC.md:2421 parenthetically calls TERM_PROGRAM_VERSION 'the version the terminal also reports when a program asks it',
without saying at launch, in the same bullet that promises survival across updates (2418-2427). SPEC_impl.md:2181-2182
similarly links the launching build's version to the emulator answer.

The checked source distinguishes the cases. host_child_env
(crates/farhelm-supervisor/src/service/core/host_launch.rs:136-160) constructs the environment once. Adoption pins the
existing child and connects to its journal; it creates the current build's emulator rather than spawning a replacement
child (hosts/terminal.rs:1770-1848). Snapshot restore creates a fresh emulator and paints its state with ReplaySeen,
discarding effects (crates/farhelm-terminal-emulator/src/snapshot/v1/mod.rs:403-425; effects.rs:199-205). Previously
delivered journal output is also ReplaySeen; previously unseen tail is ReplayUnseen; output beyond the adoption boundary
becomes Live (hosts/terminal.rs:1823-1848,2595-2655,3329-3444). Accordingly, neither restoring a snapshot nor
reinterpreting an already-delivered query sends another answer. Previously unseen queries may be answered during
recovery. A genuinely new complete XTVERSION query issued after adoption and processed live uses report_version, which
directly returns the current build constant (queries.rs:405-408; lib.rs:124-127); it does not consult a retained launch
version.

For a child launched by version A and adopted by B, the launch environment remains A while that later live query answers
B. This is expected lifetime behavior, not evidence that adoption, replay, or application compatibility is broken.
Clarify 'at launch' in the agreement prose, or explicitly say query replies follow the active supervisor's emulator. Do
not change replay rules or try to rewrite an existing child's environment to satisfy the wording. Interface claim for
g024/g049: current-emulator XTVERSION after adoption versus fixed child environment; snapshots and seen replay do not
send replies, unseen replay may.

Suggested bucket: other, documentation only. Confidence: confirmed source/spec synthesis; no runtime experiment, no
demonstrated application failure, no established security or loss-of-work route. proposed_drop: false; the existing
lifetime qualification limits the finding to this wording ambiguity rather than refuting the distinct post-adoption
query case. possible_cover: none. Implementation slices are supporting context, not claimed mandatory coverage for those
groups.

Separate-discovery preparation note, reviewed pinned tip 407a49435278844248d573083fdea6e23fd96c02. Read the authorized
complete g009/g010/g011/g024/g049 findings logs and my own complete log before searching. Latest g024/s2 independently
traces launch environment construction, existing-child adoption and current-build query response; latest g049/l1
independently traces snapshot restore, seen/unseen replay and live-query effect routing. They confirm the exact narrowed
l1 claim and explicitly classify it as documentation ambiguity only, other bucket, with no established application
failure/security/lost-work consequence. Their corroboration is not a new defect and must not be counted separately.
Existing full l1 details and qualifications are unchanged. proposed_drop remains false. possible_cover: duplicate
interface confirmations g024/s2 and g049/l1 in the explicitly authorized logs. No pending interface question remains for
this known issue.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g008/l1`, `g024/s2`,
`g049/l1`. The retained append-only histories preserve all earlier claims and corrections. Reviewed source references
name their own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no
product runtime experiments or live-install changes.
