# Reporter setup failure keeps the old Resume target

Reviewed commit: d6ba326ed8bb1de8ca178c4cd203f90a583abeeb

## TLDR

A successful named Pi or OMP restart can keep the previous conversation when preparing its reporter fails. A later
ordinary Restart can then resume that previous conversation. The conditional source route is established; occurrence is
unmeasured.

## Details

Suggested bucket: **highest**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `plans/reports/resume-target-fixes-oct22.report.md:43`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed report commit d6ba326ed8bb1de8ca178c4cd203f90a583abeeb, original report:43-45; corroborated product source at
pinned tip 463cac1528b5c5e259404975380d321a3fff0412. TLDR: after successfully using Restart with to enter conversation
B, a reporter-setup failure can leave ordinary Restart resuming prior conversation A. Source chain: proto
launch.rs:390-401 says Pi/OMP named starts report, so with hooks allowed core.rs:11417-11430 chooses
clear_capture=false. Store begin_relaunch_named:4151-4170 preserves A/source/ownership while resetting launch metadata.
Later core.rs:14418-14438 catches materialize_asset error and continues without vendor_extension;
agent_kind/mod.rs:582-609 omits the reporter. Materialization can return errors or mismatched-file refusal
(pi_extension.rs:89-114) independently of a healthy launch spec/tmux spawn. Successful publication core.rs:12526-12532
carries A's offer and11641-11677 saves edited launch settings; this is not an abort. capture.rs:275-335 refreshes A from
the store, without considering hooked, so no automatic repair occurs absent a new accepted report. On ordinary Restart
core.rs:11137-11162,11240-11245 reads A, filled_resume_argv agent_kind/mod.rs:1931-1968 substitutes A's file;
offer:1871-1888 remains Resume. verify_report_only_resume core.rs:6812-6829 only verifies A's file/id, not that this new
launch has a reporter or that B was named. relaunch_argv core.rs:3309-3342 and11963-12003 then dispatch that filled
argv. Helm launches.rs:615-623 confirms Pi --session and OMP --resume use this ordinary selector. Suggested bucket
highest (wrong-conversation resume), confidence confirmed conditional Farhelm source mechanism. Conditions: valid
pre-existing admitted locator A, different named B, hooks allowed, extension preparation error, otherwise successful
named launch, no accepted replacement report, old A still valid, later ordinary Restart. Runtime occurrence and actual
vendor execution were not observed; no vendor behavior beyond the product's selector contract is asserted. Distinct from
hooks-off invalidation and g277/f5 definitive pre-spawn rollback: this path never clears capture, never aborts, and
successfully publishes B's launch. Existing Goose/Claude-hooks-off test core.rs:20680-20690,20793-20865 covers different
branches. possible_cover: SPEC.md:2220-2229 and SPEC_impl.md:5296-5302 contain broad retain-until-report wording, in
tension with no-reporter/no-capture. Ledger:18072-18097 explicitly excludes this reporter-preparation variant from that
fix and requires disclosure, not acceptance. No applicable Planned/BUGS/queue/pending-plan match found in focused pinned
searches; rare non-first-class filter excludes wrong-conversation/user-work consequences. proposed_drop false, retaining
report provenance rather than alleging this Markdown patch introduced runtime behavior.

Provenance: `g139/f4`, `g207/f1`, `g277/f6`. These are reviewer-local identities, not additional findings. Proposed
drop: none for the retained remainder.
