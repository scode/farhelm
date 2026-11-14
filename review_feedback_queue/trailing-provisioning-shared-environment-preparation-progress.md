# Environment preparation stays on Select the recipe source in launch progress

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Environment preparation stays on Select the recipe source in launch progress

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g045/l3`.

### g045/l3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 18 (finding, open):

# Shared launch progress hides preparation behind source selection

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b

## TLDR

While a new sandbox waits for recipe preparation, the launcher says 'Select the recipe source' and omits the explanation
that preparation is needed on first use, after 24 hours or after a Farhelm update. The shared path also gives later
container launches this same misleading preparation stage. This is a progress presentation defect; creation itself can
succeed.

## Details

The newly shared environment_session_steps starts step 0 as Running (tensorlake_add.rs:515), awaits
provider.ensure_prepared (:521-529), and completes step 0 only after that await returns (:544). Preparation-needed
inserts another Running step at index 1 (:57-71), so the earlier source-selection step remains Running throughout
preparation. SessionPreparation::step puts phase text on index 1 (:77-81). The launcher's poll explicitly selects the
first Running step and copies only its name and explanation into PendingLaunch (UI list/create_form.rs:3348-3365). That
first step is named 'Select the recipe source' and has explanation None (:419-439), so both the actual preparation phase
and the first-use policy are missed. PendingLaunch::observe_stage stores those values directly (UI
launch_progress.rs:68-75), label_parts prefers stage (:104-106), and LaunchProgress displays stage_detail only when Some
(:151-164). The folded host summary independently selects the first Running step (UI provisioning.rs:878-892). The
expanded rail can display the later preparation step and its explanation; no claim is made that all surfaces lose it.

Reachability established: Tensorlake ensure_prepared calls needed when no fresh prepared snapshot exists
(tensorlake/hosts.rs:218-226), then waits for its owned preparation. The response sends the admitted host/run headers
before the final body (sessions/sandbox.rs:81-105), the client calls admitted(host,run) immediately after headers (UI
api/sandbox_sessions.rs:81-103), and the ordinary launcher stores that pair (:5922-5936) for its poll. There is no
additional progress-name rewrite. Podman likewise calls needed before its build/preparation job
(environment/podman.rs:994-1022); its launcher reaches the same poll once 350a4b33b6d4476f4a78e43cea124bc2b828f462 adds
the container UI (create_form.rs:6099-6114). Do not claim the container launcher existed before that later commit: the
ordinary Tensorlake route already demonstrates the defect at the reviewed commit.

Contract: SPEC.md Sandbox sessions :1460-1463 requires 'Preparing <recipe> for sessions…' when needed with the
first-launch/24-hour/update explanation, then named creation and Farhelm-start stages. Container sessions :4751-4753
requires corresponding recipe/place progress and that same explanation. The new stage names are generic too: inserted
'Prepare snapshot' (:289-295), 'Create the environment' and 'Connect to Farhelm in the environment' (:422-430), instead
of the specified names. This naming consequence is distinct from the first-running bug but arises from the same
replacement of the old named session plan.

Attribution checked across the pinned range: the exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a already had
named session_steps and completed/skipped step 0 before marking preparation Running (tensorlake_add.rs:930-969). Parent
of 566 likewise completed that step before preparation. The 566 patch replaces start_session_sandbox/probe/session_steps
with this generic shared plan and introduces the overlapping Running statuses. At inclusive endpoint
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, tensorlake_add.rs:515-546 and UI create_form.rs:3467-3478 retain it; the only
later tensorlake_add patch adds the unrelated seal refusal.

Suggested fix: complete/skip source selection before publishing a running preparation stage, keep one selected current
phase, and supply recipe/provider/place-aware phase names through the exhaustive wording boundary. Suggested bucket:
other (missing/misleading progress during long preparation). Could-be-highest: no security, work-loss or
destructive-action route. Confidence: confirmed source/UI/contract analysis, not observed execution. proposed_drop:
false. possible_cover: none; provided neighbour context does not report this progress issue. Caveats: only a launch
whose preparation lasts long enough for progress polling/display shows the stale phase; fresh cached preparations can
skip that boundary. Source/phase messages remain in the host's full retained rail. No runtime tests, reproduction or
live-install action performed. No unresolved source premise remains for the stated route.

Returned proposal (claims-029, event 18): **retain-verified**, suggested bucket **other**.

The ordinary sandbox launcher can select source selection throughout real preparation, hiding the preparation
explanation. The shared plan also replaces specified recipe/place labels with generic labels. This is presentation, not
a failed launch.

Original stronger consequence: Earlier lead suspected generic Prepare snapshot/Create the environment wording. Later
records establish first-running selection as well, while explicitly allowing the expanded rail to show preparation. No
security, work loss or failed creation was alleged.

Unresolved premise or limitation: None for the conditional sandbox source route. Occurrence and visible duration are
unmeasured. The container launcher was introduced later at 350a4b33b6d4476f4a78e43cea124bc2b828f462; do not claim it
already existed at 566.

Proposed basis: Record the source-confirmed progress mismatch, preserving normal cache-skip, polling-duration and
expanded-rail caveats.

Existing parent disposition (event 18): verified. Parent explicitly accepted named retain-verified proposal from
claims-029 after inspecting it. The ordinary sandbox launcher can select source selection throughout real preparation,
hiding the preparation explanation. The shared plan also replaces specified recipe/place labels with generic labels.
This is presentation, not a failed launch. Record the source-confirmed progress mismatch, preserving normal cache-skip,
polling-duration and expanded-rail caveats. Checked source/results: ["56659839785691f76da3af5d8f51b511cff4ba8b,
provisioning/tensorlake_add.rs:57-81,419-439,515-546: preparation inserts Running index 1 while index 0 remains Running
until ensure_prepared returns; index 0 has no explanation.","Same revision, tensorlake/hosts.rs:218-226: an ordinary
stale/missing prepared snapshot calls needed before awaited preparation. environment/podman.rs:994-1022 has the
corresponding provider callback.","Same revision, sessions/sandbox.rs:81-105, UI api/sandbox_sessions.rs:81-103 and
list/create_form.rs:5922-5936: admission headers reach the pending launch before final completion.","Same revision, UI
list/create_form.rs:3348-3365 selects the first Running step; launch_progress.rs:68-75,104-106,151-164 renders that
selected name/detail. UI provisioning.rs:878-892 independently selects the first Running step. The expanded rail retains
the later explanation.","Exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a tensorlake_add.rs:932-969 retires
source selection before preparation; endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db tensorlake_add.rs:515-546 and UI
create_form.rs:3467-3478 retain the overlap.","At reviewed 566 commit SPEC.md:1457-1463,4749-4753 requires
recipe/provider/place names and preparation explanation. Current SPEC.md:1473-1478,4818-4823 retains that contract. No
runtime observation."] Current coverage/results: ["Both specifications searched for preparation, named launch progress,
first-launch and first-running. SPEC_impl.md prepared-snapshot and sandbox-session sections explain the shared task;
they do not accept hidden or generic launch stages.","TODO.md Planned empty; plans/queue/INDEX.md empty. The matching
sandbox-sessions and podman-environments plans are already landed (plans/REPORTS.md), so their requirements are not
unlanded fix coverage.","BUGS.md, queue INDEX and active TRIAGE_OUTCOMES.md searched for preparation progress, source
selection, first-running and launch-progress wording: no exact item. The queued quick-launch preparation-refusal item
concerns a failed prerequisite read, not this running stage.","FILTER.md rare self-correcting diagnostic/display filter
does not fully match: ordinary first/stale preparation is a normal trigger, although display requires preparation to
last through polling."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved:
"Earlier lead suspected generic Prepare snapshot/Create the environment wording. Later records establish first-running
selection as well, while explicitly allowing the expanded rail to show preparation. No security, work loss or failed
creation was alleged."; "None for the conditional sandbox source route. Occurrence and visible duration are unmeasured.
The container launcher was introduced later at 350a4b33b6d4476f4a78e43cea124bc2b828f462; do not claim it already existed
at 566.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g045 record](records/review-through-00de0b6c/groups/g045.json). Record ID: `l3`.
