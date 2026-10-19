# Trailing code review checkpoint

Last reviewed commit: `c5fcfee8681e46b1d47718771b321488037d3f83`

This is the exclusive starting point for the next trailing code review of `main`. Advance it only after completing the
procedure in [AGENTS.md](AGENTS.md#trailing-code-review), together with any resulting queue updates. A proposed
checkpoint reaches `main` only when its review-result PR is merged.

## Review through c5fcfee8

Reviewed range: `aeeda466fdaee55755a341046cb051e882495dcd..c5fcfee8681e46b1d47718771b321488037d3f83`, 41 commits. The
tip and saved checkpoint came from fetched `origin/main`; the checkpoint was verified as an ancestor. A second fetch
before recording the result found no newer checkpoint or main commits. Five separate same-model adversarial reviewers
read the complete individual patches, including intermediate changes and deletions:

| Ordered commits | Count | Main content                                                         |
| --------------- | ----: | -------------------------------------------------------------------- |
| 1–10            |    10 | Hover summaries, release-review page, readiness tests and menu focus |
| 11–19           |     9 | Template shortcut refactoring, launcher application and Cmd-J        |
| 20–28           |     9 | Plan state, landing reports, TODO and triage decisions               |
| 29–36           |     8 | Approved-plan deletions and shortcut documentation                   |
| 37–41           |     5 | Triage criteria and plan reports                                     |

All 41 commits are accounted for; no assigned patch lines remain unread. Surrounding code and coverage sources were read
selectively to assess candidates, not exhaustively across the repository. Historical private runtime evidence and
maintainer conversations quoted by reports were not independently authenticated.

Seven records were retained. Three are conservatively in `highest`: a main hover summary that omits observed YOLO, a
template-list row that can launch a template recreated under the same name, and a cached shortcut badge that can guide a
later launch to a different template. Their static mechanisms are established, but consequential reliance, complete
concurrent sequences and harmful launches remain unverified. The two template-list records preserve specification
ambiguities rather than claiming fresh resolution or preference caching is itself forbidden. The independent auditor
agreed that stale badge display alone is expressly accepted, but could not approve excluding the possible later
wrong-target gesture. Triage should resolve those contracts before commissioning broader designs.

One `high` record concerns reapplying an unchanged template after a lost reply, which clears the retained retry identity
and can duplicate a launch. Three `other` records concern inaccessible lower hover-summary rows, shortcut tests that
skip remaining cleanup after an earlier cleanup failure, and unsafe links in the release-review page generator. The last
has a confirmed attribute-escaping defect, but attacker exposure and consequential security impact are unverified; its
input is curated maintainer prose. Exact confidence, possible coverage and verification limits are in each record.

A separate fresh-context auditor checked every proposed security or user-work exclusion, including leads refuted by
source inspection. It agreed to duplicate-only exclusions for tunnel port reuse, armed-sequence binding drift and header
confirmation focus recovery, all of which remain recorded already. It confirmed the explicitly accepted same-entry host
retarget/adoption behavior, the scoped host-identity redesign criteria, DOM-only tooltip rendering, modal/opening
fences, inert historical template trust, missing-name refusal and whole-batch validation. These are narrow conclusions,
not clean verdicts for their subsystems. Ambiguous exclusions became the template-list and release-page records above.

The reported native shortcut desynchronization was narrowed: a refused Begin can leave an orphan press snapshot and drop
a later press, but a following press in a stable context can recover. The landing report's stronger claim that it
persists until reload was not substantiated. The recoverable narrow admission race matched the existing filter. No
runtime reproduction was run.

This was a static review. No product runtime, browser, native Mac or vendor-agent tests were run, and no prior runtime
results were reused as current passes. The result changes feedback records, the index and this checkpoint only. No fixes
or triage outcomes were executed. Documentation formatting, whitespace, queue/index consistency and instruction pointer
checks validate the result. The review-result PR stays a draft for separate manual merging; this proposed checkpoint has
not advanced on main.

## Review through aeeda466

Reviewed range: `afc828776596f2e6639d16d4203ccc8b3d0f5862..aeeda466fdaee55755a341046cb051e882495dcd`, 164 commits. The
tip and checkpoint came from fetched `origin/main`; the saved checkpoint was verified as an ancestor. Eight primary
reviewers covered contiguous slices of the ordered commit list. Three more reviewers took the large trust and wrapper
patches so every assigned patch, including intermediate changes and deletions, was read in bounded chunks. Their stated
coverage accounts for all 164 commits with no unread patch lines. Surrounding code, specifications, and coverage sources
were read for candidates, not exhaustively across the repository.

Twelve findings were retained: ten conservatively placed in `highest` for possible security, wrong-target or
authorization consequences, and two in `other` for shortcut persistence and false success. These are source-level
findings with their own confidence limits, not twelve reproduced bugs. The most consequential paths involve workspace
trust, launch and prompt approvals, localhost tunnels, and file opening. A separate fresh-context auditor checked the
proposed exclusions with possible security or user-work impact. It agreed with the exact duplicate, accepted-behavior
and code-refutation exclusions; four exclusions were ambiguous and became records with their possible cover stated. No
finding was fixed and no triage decision was made.

No runtime, browser, native Mac or vendor-agent test was run for this static review. The review-result change contains
feedback records, the index and this checkpoint. Its draft PR is for separate review and merging; the checkpoint does
not advance on `main` until that PR lands.

## Completed review

Reviewed range: `991835f826b25bc93eb088f6d3984477a9dc2fab..afc828776596f2e6639d16d4203ccc8b3d0f5862`, 139 commits. The
tip and its saved checkpoint were fetched and pinned before review; the checkpoint was verified as an ancestor. Eight
separate adversarial reviewers read every assigned commit's complete patch, including deleted plans, reports, findings
and release fragments. Assignment counts were checked against the complete pinned commit list:

| Scope                                                     | Commits |
| --------------------------------------------------------- | ------: |
| Earlier landed-plan approvals                             |      28 |
| Later landed-plan approvals                               |      13 |
| Plan creation, lifecycle, reports and review instructions |      59 |
| Documentation, release notes, specifications and triage   |      21 |
| Trash, menu focus, scrolling, IME, downloads and outages  |       7 |
| Registry retry namespace, Git policy and trust startup    |       4 |
| Agent deletion and initial-prompt backend                 |       2 |
| Prompt UI, template names and Replace retries             |       5 |

No assigned patch content remains unread. Unchanged surrounding implementations, specifications and coverage sources
were read selectively. This was not an exhaustive review of those surrounding modules. Private historical runtime
evidence was unavailable and was not authenticated. Installed OMP 18.10.4 argument parsing was inspected statically;
other vendor implementations and native input-method behavior were not independently verified.

Two records were retained. One likely `other` finding concerns outage cards whose unbounded vertical centering can hide
their explanation and remedy above the reachable area. Browser geometry was not reproduced. The other conservatively
retains a potentially highest-priority residual menu-focus case: independent focus during pending arrow navigation can
still be overridden before subsequent activation. The historical plan explicitly accepted this exact limitation, but the
named specification and triage coverage sources do not reflect that narrower acceptance. An independent auditor could
not unambiguously approve exclusion under the recording rules. Its record preserves the exact historical decision and
calls for reconciling it before commissioning a broader design; it does not claim the implementation ignored the
maintainer's direction. Destructive activation was not reproduced.

Three additional independent auditors checked proposed exclusions with possible security or user-work consequences. They
agreed to the scoped refutations for trash-answer identity, registry namespace separation, deletion consent and
connection fencing, prompt placement and replay, old-client prompt decoding, template-name validation, download reply
correlation, terminal recovery ownership, and IME target isolation. They also checked startup ownership across
cancellation, Git repository-environment scrubbing and trust-removal admission. These are static conclusions about the
named mechanisms, not general clean verdicts for their subsystems.

The exclusions for stalled Claude trust publication and residual trust after failed cleanup rest on exact accepted
limits and recorded decisions. A further audit rejected the blanket claim that every failed-create rollback confirms all
work stopped: a failed database confirmation plus a failed process sweep can still lose its session record after tmux
termination. A different independent auditor agreed to exclusion only because the existing ledger records that exact
case and SPEC.md accepts coinciding independent faults leaving work running beside a relaunch. The UI discards the retry
key; another user action is still needed to launch again. This does not accept false refusal proof after a single
ordinary failure, cross-host effects or database corruption.

Review and validation were static. No product runtime suite, browser reproduction, real agent turn or live-installation
action was run, and no previous product-suite results were reused as current passes. Formatting, whitespace, queue/index
consistency, complete commit assignment and instruction-pointer checks validated the records. No fixes or triage
outcomes were executed. The review-result change was prepared as a draft for manual merging.

## Astra high repeat review

The same range was reviewed again: `991835f826b25bc93eb088f6d3984477a9dc2fab..afc828776596f2e6639d16d4203ccc8b3d0f5862`,
139 commits. The first result landed as `78394ca6b42d698eb6566654371b27caf02dca54` in
[PR #2213](https://github.com/scode/farhelm/pull/2213/changes) before this repeat. The checkpoint remains at
`afc828776596f2e6639d16d4203ccc8b3d0f5862`; neither that result commit nor later main changes belong to this review.

Eight fresh reviewers used `gpt-6-astra` with high reasoning, the unchanged charter and test checklist, the same eight
commit assignments in the table above, and the same original checkout snapshot. They received no first-pass results. All
139 individual patches, including deletions and intermediate changes, were read. Five fresh Astra high auditors then
checked proposed exclusions and substantive follow-ups. Primary assignments matched; follow-up audit work depended on
the reported candidates. These results describe this one static-review sample.

The first pass retained two records. Astra independently reported both and retained six additional records after
coverage checks: four source-level issues and two unresolved questions. The table counts records, not reproduced bugs.

| Concern                                                    | First pass                                               | Astra high repeat                                                                              |
| ---------------------------------------------------------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Outage-card text outside reachable area                    | Retained, likely `other`                                 | Independently repeated; provenance added                                                       |
| Menu arrows overriding independent focus                   | Retained, acceptance gap                                 | Repeated; added indefinitely unacknowledged focus-request case                                 |
| Launcher retaining a settled failed key                    | Not recorded                                             | Added `high`; static mechanism independently confirmed                                         |
| Separate Replace YOLO questions                            | Not recorded                                             | Added `other`; static state/admission path, live sequence unverified                           |
| Archive recovery clearing replacement-path trust           | Not recorded                                             | Added `other`; sequential reuse path, vendor sequence unverified                               |
| File disclosure obscured by approval cards                 | Not recorded                                             | Added conservative `highest`; two source audits, overlap/transfer unverified                   |
| Refusal proof following incomplete process cleanup         | Excluded for the concrete accepted coincident-fault case | Retained conservative `highest` as unresolved coverage of the new signal                       |
| OSC filename visual distinction, especially trailing space | Not recorded                                             | Retained provisional `highest` as an unresolved lead; no concrete harmful transfer established |

The additional launcher and confirmation records concern omissions adjacent to the Replace fixes. The archive-recovery
record concerns a successful pending-archive reconciliation path that lacks the retired-record branch's reused-path
protection. The disclosure-occlusion record comes from independently investigating a suspected archival follow-up;
geometry-only admission does not check whether higher approval cards hide the disclosure. These are source findings with
individual confidence limits, not claims that all were introduced by commits in the reviewed range.

The two additional uncertainty records need separate interpretation. All concrete failed-cleanup examples found already
have ledger decisions, and SPEC.md accepts coinciding faults leaving work beside a relaunch. The first pass excluded a
specific combination on that basis; Astra's auditor did not establish that every route qualifying for the new signal
necessarily meets that exception. No uncovered single-fault execution was established. The filename lead has low
confidence as a security defect: no collision plus harmful transfer was demonstrated, and the escaping contract does not
promise universal Unicode distinguishability. Both remain recorded because the independent exclusion verdict was
ambiguous, not because their strongest security interpretation was proved.

Independent exclusions agreed to the precise repaired or accepted boundaries for registry scoping, deletion consent,
one-time prompt composition/replay/storage, old-client decoding, template-name rendering, trash confirmations,
non-displacing recovery, peer-text rendering, Git selectors and trust worker ownership. They also checked the retained
human release-publication fence, qualified guarded-mode wording and destructive preconditions under stale views.
Historical proxy, manually retargeted trash symlink, split-link heuristic and deliberate devbuild-environment cases
matched their exact recorded acceptances. A raw local desktop identity refusal remains a filtered diagnostic issue; that
conclusion does not accept a remote authorization bypass. None of these exclusions is a general subsystem verdict.

No assigned patch gaps remain. Surrounding modules and coverage sources were inspected selectively; the disclosure
follow-up additionally inspected the full terminal-file asset, relevant terminal/approval ancestors and click paths.
Browser/native behavior, vendor parsing and historical private runtime evidence remain unverified. No product runtime,
browser or vendor-TUI tests were run, and no earlier runtime result was reused as a current pass. The change contains
queue records, provenance and this comparison only; no fixes or triage decisions were executed. The repeat result is a
draft PR for separate review and merging.
