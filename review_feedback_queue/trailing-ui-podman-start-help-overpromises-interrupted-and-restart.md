# Container-start explanation promises interrupted status for every session

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: when Farhelm starts a stopped container, its explanation says the sessions return as
interrupted with Restart available, even for sessions that already ended and must retain their known result. This
misstates recovery; the backend's classification remains correct. Source: ProviderUi::starting_help at
environment_ui.rs:261-269 says "Starting the container again; its sessions return as interrupted, with Restart
available." resume_help :249-252 also uses it for stopped-container Resume. sandbox_sessions.rs:124-130 installs this in
the unavailable card for starting:true and :345 uses resume_help for Stopped. session_view.rs:2022-2024 chooses that
notice for stale/frozen owned hosts and :2357-2363 mounts Controls whenever sandbox_host exists, without excluding
already-ended sessions. The final direct transition check settles the shipped route: environment/podman.rs:643-677
resolves lifecycle_verb from actual container status and sets starting: verb == "start"; :682-704 performs start under
the ordinary hold through environment/lifecycle.rs:68-80. manager.rs:3841-3890 starts a Stopped environment once its
place is connected independently of ended session outcomes. A retained known-exited session therefore sees the
unconditional recovery text during that automatic start or on its explicit Resume control. Exact contract,
SPEC.md:4786-4789: "Successful start brings back its supervisor with ordinary session classification. A host reboot
interrupts sessions last known live, while known exits keep their outcome. A container stopped and started on the same
host boot returns ended sessio

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g077/f6`.

### g077/f6

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 17 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. Discovery pass 1. TLDR: when Farhelm starts a stopped
container, its explanation says the sessions return as interrupted with Restart available, even for sessions that
already ended and must retain their known result. This misstates recovery; the backend's classification remains correct.
Source: ProviderUi::starting_help at environment_ui.rs:261-269 says "Starting the container again; its sessions return
as interrupted, with Restart available." resume_help :249-252 also uses it for stopped-container Resume.
sandbox_sessions.rs:124-130 installs this in the unavailable card for starting:true and :345 uses resume_help for
Stopped. session_view.rs:2022-2024 chooses that notice for stale/frozen owned hosts and :2357-2363 mounts Controls
whenever sandbox_host exists, without excluding already-ended sessions. The final direct transition check settles the
shipped route: environment/podman.rs:643-677 resolves lifecycle_verb from actual container status and sets starting:
verb == "start"; :682-704 performs start under the ordinary hold through environment/lifecycle.rs:68-80.
manager.rs:3841-3890 starts a Stopped environment once its place is connected independently of ended session outcomes. A
retained known-exited session therefore sees the unconditional recovery text during that automatic start or on its
explicit Resume control. Exact contract, SPEC.md:4786-4789: "Successful start brings back its supervisor with ordinary
session classification. A host reboot interrupts sessions last known live, while known exits keep their outcome. A
container stopped and started on the same host boot returns ended sessions through ordinary exit classification.
Captured resumable conversations offer Restart, and agents never restart automatically." Supervisor source corroborates
it: service/core.rs:6993-7010 compares boot IDs; :7083-7098 converts only Launching, Running and StopRequested to
Interrupted after reboot, preserving known exit outcomes. The reload contract :6817-6826 preserves status/codes for
already-exited sessions. The new Podman wording is introduced in this group's assigned new file, not merely an unrelated
historical implementation. Comparing environment_ui.rs from this commit to pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db confirms starting_help/resume_help remain unchanged. Suggested bucket: other,
inaccurate recovery/status guidance. Could-be-highest:no established security, data-loss, process-loss or
unauthorized-restart route; no action is taken from the wording and agents do not automatically restart. Confidence:
confirmed source/contract analysis, not runtime observation. proposed_drop:false. possible_cover:none in supplied
neighbour projection; no prohibited coverage searches. Fix direction: say Farhelm is starting the container and
reconnecting, and defer exact session outcome/Restart availability to actual classification and captured conversation;
avoid promising either preserved processes or universally interrupted sessions. All readily checkable
route/classification/persistence premises settled. No runtime tests, reproductions or live-install actions.

Returned proposal (claims-050, event 17): **retain-verified**, suggested bucket **other**.

The starting-container explanation universally promises Interrupted and Restart although known exits retain their
outcome and Restart depends on captured resumable conversation. Backend classification remains correct; the established
defect is guidance.

Original stronger consequence: The lead and final finding concern inaccurate recovery/status guidance only. They
expressly preserve correct backend classification and deny any established automatic restart, data-loss or security
route.

Unresolved premise or limitation: No runtime frequency or duration measurement; no unverified premise remains for the
conditional wording mismatch.

Proposed basis: Retain the universal Interrupted/Restart promise as an ordinary guidance defect, including known-exit
and captured-conversation caveats.

Existing parent disposition (event 17): verified. Parent explicitly accepted named retain-verified proposal from
claims-050 after inspecting it. The starting-container explanation universally promises Interrupted and Restart although
known exits retain their outcome and Restart depends on captured resumable conversation. Backend classification remains
correct; the established defect is guidance. Retain the universal Interrupted/Restart promise as an ordinary guidance
defect, including known-exit and captured-conversation caveats. Checked source/results:
["350a4b33b6d4476f4a78e43cea124bc2b828f462:crates/farhelm-ui/src/environment_ui.rs:249-267 reuses unconditional
starting_help for stopped Resume. sandbox_sessions.rs:124-130 installs it for starting:true; :345 uses resume_help.
session_view.rs:2022-2024 and :2357-2363 do not exclude known-exited sessions from those host surfaces.","Same
revision:crates/farhelm-helm/src/environment/podman.rs:667-704 derives starting:true from actual start and performs it.
manager.rs:3841-3889 automatically starts a stopped environment without inspecting ended session outcomes.","Same
revision:SPEC.md:4782-4789 preserves known exits and ordinary same-boot classification.
crates/farhelm-supervisor/src/service/core.rs:6993-7010 compares boots; :7085-7097 converts only
Launching/Running/StopRequested on reboot. :6817-6826 documents retained exited outcomes. The environment_ui.rs
review-tip diff leaves starting_help unchanged.","Conditional shipped route: a retained known-exited session in a
stopped container sees this during start or via its stopped Resume tooltip. No runtime observation, wrong backend
status, automatic restart or process-loss route is claimed."] Current coverage/results: ["Current SPEC.md Container
sessions retains ordinary classification and conditional Restart. SPEC_impl.md Podman lifecycle and supervisor startup
contracts were searched/read where relevant; no universal Interrupted promise is accepted.","TODO.md Planned empty.
Searched unlanded plans/queue, BUGS.md, INDEX.md and active TRIAGE_OUTCOMES.md for
starting-container/interrupted/Restart wording. No exact coverage. The originating podman-environments plan is already
landed.","FILTER.md rare-diagnostic boundary does not clearly match: the wording occurs on ordinary stopped-container
starts, including complete evidence of a known exit. The session crash/incomplete-recovery filter does not establish
acceptance of universal guidance when actual known outcomes are preserved. Retain."] Exact cited cover: [] Original
stronger consequence and missing premise remain preserved: "The lead and final finding concern inaccurate
recovery/status guidance only. They expressly preserve correct backend classification and deny any established automatic
restart, data-loss or security route."; "No runtime frequency or duration measurement; no unverified premise remains for
the conditional wording mismatch.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g077 record](records/review-through-00de0b6c/groups/g077.json). Record ID: `f6`.
