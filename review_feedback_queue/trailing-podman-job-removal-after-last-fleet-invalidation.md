# Container job completion lacks a final fleet invalidation

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: a successful status refresh can leave Preparing and a disabled Rebuild button after
container image work finishes, until an unrelated invalidation, changed place observation or remount supplies another
read. Source: place_view selects building from podman_jobs membership (tensorlake/build.rs:605-615). prepare_images
bumps events before returning (environment/podman/build.rs:288), and success enters Ready before return (:404); the job
remains registered for both. start_podman publishes its watch result (:718-723), then PodmanJobAdmission::drop removes
the entry (:446-448), with no fleet bump afterward. The Rebuild HTTP route returns admission/202 (hosts.rs:4402-4407). A
successful GET between final invalidation and registry removal reads building=true with no later completion
invalidation. FleetEvents carries only a revision hint (feed.rs:14-30). The normal final-bump-to-drop sequence has no
await, so this needs a narrow concurrent/preemption schedule; it is not claimed on every completion. The CLI runtime is
explicitly multithreaded (crates/farhelm/src/main.rs:2672-2677), so other request tasks are not excluded by that lack of
await. Frequency and duration are unmeasured. GUI: RecipeList at 350a4b3 restarts place_status on fleet notices or
changed successful place observations (tensorlake/recipes.rs:367-385); RecipePlaces disables Rebuild on row.building
(containers_settings.rs:257-260). Its POST-completion on_refresh (:270-279) follows admission and ordinarily precedes
image work. Stable place observations do not rescue this: environment_ui.rs:684-724 polls only during in-

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g061/l5`.

### g061/l5

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 16 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b; caller at 350a4b33b6d4476f4a78e43cea124bc2b828f462; affected
backend and GUI status paths unchanged through pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: a successful
status refresh can leave Preparing and a disabled Rebuild button after container image work finishes, until an unrelated
invalidation, changed place observation or remount supplies another read. Source: place_view selects building from
podman_jobs membership (tensorlake/build.rs:605-615). prepare_images bumps events before returning
(environment/podman/build.rs:288), and success enters Ready before return (:404); the job remains registered for both.
start_podman publishes its watch result (:718-723), then PodmanJobAdmission::drop removes the entry (:446-448), with no
fleet bump afterward. The Rebuild HTTP route returns admission/202 (hosts.rs:4402-4407). A successful GET between final
invalidation and registry removal reads building=true with no later completion invalidation. FleetEvents carries only a
revision hint (feed.rs:14-30). The normal final-bump-to-drop sequence has no await, so this needs a narrow
concurrent/preemption schedule; it is not claimed on every completion. The CLI runtime is explicitly multithreaded
(crates/farhelm/src/main.rs:2672-2677), so other request tasks are not excluded by that lack of await. Frequency and
duration are unmeasured. GUI: RecipeList at 350a4b3 restarts place_status on fleet notices or changed successful place
observations (tensorlake/recipes.rs:367-385); RecipePlaces disables Rebuild on row.building
(containers_settings.rs:257-260). Its POST-completion on_refresh (:270-279) follows admission and ordinarily precedes
image work. Stable place observations do not rescue this: environment_ui.rs:684-724 polls only during in-flight probing
or feed fallback. Tip checks found no diff in RecipeList/RecipePlaces and the complete use_environment_places function
retains that behavior. Distinction from caller-supplied g060/l1: failed place_status renders only an error
(containers_settings.rs:213-215); Retry loading invokes the separate definition reload (tensorlake/recipes.rs:540-545).
That defect can keep a failed GET unresolved even with a correctly ordered completion event. This finding instead admits
a successful stale GET: retry alone does not supply the missing completion invalidation, and moving the event alone does
not fix a failed read. Preserve both causal routes without counting the failed-refresh symptom as new here. Fix
guidance: remove admission before final fleet invalidation, owning panic/cancellation publication consistently. Existing
Tensorlake completion does that (56040bb964270bb40d191902173dbb84b3ba78fc, tensorlake/build.rs:642-653). Suggested
bucket: other, unchanged. Confidence: confirmed source defect and available concurrent route, not runtime-observed
behavior. proposed_drop: false. possible_cover: g060/l1 is related but concerns failed read recovery; other reviewers
may have this completion-feed defect, but no such findings were searched. Could-be-highest: no security/user-work loss.
Original Preparing/disabled-button consequence retained with the narrow-window caveat; no priority lowering. Original
early-prerequisite silent-failure suspicion remains unclaimed separately because place-observation refresh may expose
probe failure. No tests or reproductions performed.

Returned proposal (claims-039, event 16): **retain-verified**, suggested bucket **other**.

The last fleet event precedes registry removal. A successful concurrent status read can spend that event on
building=true and leave Preparing/Rebuild disabled until another read trigger. The schedule is narrow, unmeasured and
source-supported.

Original stronger consequence: Preparing and a disabled Rebuild button can remain after image work completes until
unrelated invalidation, changed place observation or remount. Original early-prerequisite silent failure was
investigated but not claimed separately. No security or user-work-loss consequence was proposed.

Unresolved premise or limitation: No runtime frequency or duration measurement. No claim every completion leaves stale
state. The exact filter interpretation of an indefinitely retained display plus disabled command remains for
parent/triage.

Proposed basis: Retain other with the narrow concurrent-read schedule, source-only confidence, distinct failed-read
route and explicit filter ambiguity.

Existing parent disposition (event 16): verified. Parent explicitly accepted named retain-verified proposal from
claims-039 after inspecting it. The last fleet event precedes registry removal. A successful concurrent status read can
spend that event on building=true and leave Preparing/Rebuild disabled until another read trigger. The schedule is
narrow, unmeasured and source-supported. Retain other with the narrow concurrent-read schedule, source-only confidence,
distinct failed-read route and explicit filter ambiguity. Checked source/results:
["56659839785691f76da3af5d8f51b511cff4ba8b:tensorlake/build.rs:605-615 computes building from podman_jobs membership;
718-724 publishes result before PodmanJobAdmission::drop:446-448 removes the entry, with no following fleet
event.","Same revision: environment/podman/build.rs:288 bumps before returning; success enters Ready at 404 while
admission remains held. No await separates normal final return and admission drop, so concurrent handling/preemption is
required.","350a4b33b6d4476f4a78e43cea124bc2b828f462:farhelm-ui/src/tensorlake/recipes.rs:367-385 restarts the status
resource on fleet notices/changed successful place observations; containers_settings.rs:213-215 only displays
status-read error, 234-260 derives Preparing/disabled Rebuild, and 270-279 refreshes after admission reply rather than
after job completion.","Reviewed tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:farhelm-ui/src/environment_ui.rs:948-992
polls only during probing/feed fallback. The record's old 684-724 citation shifted; the actual tip function is at 952.
This supports lack of a guaranteed quiet-state reread.","The supplied g060/l1 failed-read recovery defect is distinct:
this route requires a successful stale GET. The initial silent-prerequisite-failure suspicion remains unclaimed because
place observation can expose it. No runtime observation."] Current coverage/results: ["Current SPEC.md recipe
build-status contract at 450-483 and SPEC_impl.md:6025-6056 describe per-place readiness/registry projection; neither
accepts missing completion invalidation.","TODO Planned empty; BUGS and active TRIAGE_OUTCOMES searches found no
matching completion-feed decision. No unlanded plan states removal-before-final-invalidation work; landed
podman-environments scope is not future coverage.","Full INDEX.md and preparing/build-completion/feed searches found no
exact queue item. orphan-cleanup-start-does-not-invalidate-feed.md concerns another operation's start event, not this
completion registry ordering.","FILTER.md rare self-correcting display rule might be argued to cover remount recovery,
but its 'briefly wrong' boundary is unclear when the state can persist indefinitely and disables Rebuild. The
sub-second-person-action filter does not match an automatic GET. Recommend retention on that ambiguity, not filtering
solely because the window is narrow."] Exact cited cover: [] Original stronger consequence and missing premise remain
preserved: "Preparing and a disabled Rebuild button can remain after image work completes until unrelated invalidation,
changed place observation or remount. Original early-prerequisite silent failure was investigated but not claimed
separately. No security or user-work-loss consequence was proposed."; "No runtime frequency or duration measurement. No
claim every completion leaves stale state. The exact filter interpretation of an indefinitely retained display plus
disabled command remains for parent/triage.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g061 record](records/review-through-00de0b6c/groups/g061.json). Record ID: `l5`.
