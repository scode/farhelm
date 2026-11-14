# Agent environment launches enter GUI place-default and recent-history bookkeeping

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Agent environment launches enter GUI place-default and recent-history bookkeeping

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g020/l5`, `g050/l1`.

### g020/l5

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 17 (finding, open):

# Agent environment launches enter GUI convenience bookkeeping

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435

## TLDR

A successful environment launch requested by an agent can change the container host preselected in the GUI and push the
user's own recent setups out of its history. The new agent environment route carries enough trusted target data to
update history, but being a valid target does not make it a GUI choice.

## Details

This is the agent consumer of caller-supplied g050/l1 line8, independently checked here; it is not a newly claimed
storage root cause. The assigned new path at agent_requests.rs:4477-4484 creates CreateReq::for_agent_environment, and
:4541-4548 dispatches it to start_session_environment. sessions.rs:1192-1253 retains its agent marker and origin()
returns CreateOrigin::Agent. provisioning/tensorlake_add.rs:357-361 nevertheless attaches environment_target
independently; the launch task sends that same request into create_in_session_sandbox at :1007-1013.
sessions.rs:1756-1802 passes it to create_session_request, whose :1848-1863 retains origin before taking the private
approved launch; :1893-1894 separately captures environment_target. do_create_session_with_environment at :2055-2060
correctly withholds explicit_selection for agents, but accept_created_session/record_created_history at :2144-2242 still
pass environment_target into record_create_history_with_environment_if_current. The live-claim closure explicitly admits
a session-owned host when environment_target exists. There is no agent-origin input left to distinguish this target from
a raw command selected in the GUI.

store.rs:8835-8890 admits the current claim and :9015-9018 calls environment_history::record with target independently
from optional selection. store/environment_history.rs:49-54 accepts a target with None selection, :60-75 inserts its
admission marker, :82-88 overwrites last_container_host, and :93-99 ages the same bounded window. The reader :111-114
hides None-selection rows but does not undo their eviction of structured GUI history. Current-claim and fresh-admission
checks certify the successful create and prevent replay, not GUI provenance. A newly created environment with a fresh
session supplies the normal successful admission.

Exact contract at SPEC.md:5158-5163: 'Only what the user explicitly selects in the GUI may affect the GUI’s future
defaults and suggestions' and 'anything an agent creates do not qualify, because none of them is a choice the user made
in the GUI.' The user's per-request Allow is approval of an agent-selected launch, not the GUI-launch provenance that
explicit_selection continues to enforce. Preserve agent origin through convenience-bookkeeping admission or suppress
these GUI effects for the agent environment consumer, while keeping GUI command launches eligible despite their None
structured selection.

Suggested bucket: other (wrong future place default and eviction of convenience suggestions, no established security or
loss of user-owned work). Confidence: confirmed by pinned source and contract; no execution. Could-be-highest: no
established route. proposed_drop: false. possible_cover: same underlying provenance issue as g050/l1 line8, parent owns
deduplication; this record identifies the assigned new agent entry point's integration. Caveats: needs successful
approved create on a current host connection; enough successful creates are needed to evict the whole recent window.
Distinct from g020/l1 physical-host consent and g020/l3 preflight timing. Original interface consequence retained. No
unresolved source premise remains for this consumer.

Discovery follow-through: compared the complete in-range path history and the integrating revision through pinned
endpoint. store/environment_history.rs has no later endpoint diff; assigned agent environment plan/dispatch/ordinary
clone production code is likewise unchanged after 190d844 through 00de (later agent_requests edits are
environment-delete commentary/tests and terminal-kind fixture shape). The GUI-provenance contract was read in its full
governing SPEC.md section, Remote input, session defaults, and availability (5145-5199), delivered through
discovery-contract-manifest.json. It explicitly permits some hostile-host view crowding/availability effects but
separately excludes agent-created launches from GUI-choice provenance; the current-connection history guard does not
change that distinction. This remains a confirmed interface overlap with g050/l1, not a second independently novel
underlying issue. No runtime checks.

Returned proposal (claims-010, event 17): **retain-verified**, suggested bucket **other**.

Successful approved agent environment creates carry a reusable target into GUI convenience bookkeeping after agent
origin has been reduced to None selection. The target alone updates the remembered container place and ages the shared
recent window. This is distinct from approval authority and SSH-key admission.

Original stronger consequence: Wrong future container-place default and eviction of GUI recent setups; no security or
lost user-owned work claimed. The history explicitly identifies overlap with externally supplied g050/l1 rather than
claiming an independently novel storage cause.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain as source-verified other. Use underlying_key to reconcile the supplied g050/l1 overlap in the
parent; that record is outside this named batch and was not independently inspected here, so aliases is intentionally
empty.

Existing parent disposition (event 17): verified. Parent explicitly accepted named retain-verified proposal from
claims-010 after inspecting it. Successful approved agent environment creates carry a reusable target into GUI
convenience bookkeeping after agent origin has been reduced to None selection. The target alone updates the remembered
container place and ages the shared recent window. This is distinct from approval authority and SSH-key admission.
Retain as source-verified other. Use underlying_key to reconcile the supplied g050/l1 overlap in the parent; that record
is outside this named batch and was not independently inspected here, so aliases is intentionally empty. Checked
source/results: ["190d844e58c96cf197aa838d0b0eea8062f48435: sessions.rs:1192-1253 preserves an agent-origin marker;
provisioning/tensorlake_add.rs:357-361 separately attaches environment_target; agent_requests.rs:4477-4550 dispatches
that request.","Same revision: sessions.rs:2055-2060 deliberately gives agents no explicit_selection, but
record_created_history:2180-2242 still passes environment_target and accepts a current session-owned connection when
that target exists.","Same revision: store.rs:9015-9018 invokes environment_history::record independently of selection.
store/environment_history.rs:39-114 accepts None selection, inserts an admission, overwrites last_container_host, evicts
beyond MAX_LAUNCH_HISTORY, then hides None-selection rows from reusable setup reads.","git log of the supplied range
includes environment history's introducing 52cc5d72 and the new agent consumer's 190d844e. A successful
current-connection create is required, and sufficient repeated creates are needed for broad suggestion eviction. No
runtime reproduction or measured frequency."] Current coverage/results: {"SPEC.md":"Remote input, session defaults, and
availability:5122-5127 expressly excludes anything an agent creates from GUI future-default/suggestion provenance.
Container sessions:4873-4875 retains environment choices after Delete, without changing that origin
rule.","SPEC_impl.md":"Shared recipes and prepared artifacts:6020-6023 calls this captured request intent. Composer
history:5054-5087 permits raw command launches to age a shared window, but agent-launch there describes launch kind, not
agent-request origin; it does not explicitly accept this agent-origin effect.","TODO.md
Planned":"Empty.","plans/queue":"INDEX.md empty; no unlanded repair.","BUGS.md":"No matching acceptance.","queue":"INDEX
and environment-history/default searches found no exact item. sandbox-launch-skips-remembered-agent-choices is the
opposite GUI-origin omission, not coverage for agent-origin admission.","TRIAGE_OUTCOMES.md":"Targeted
history/default/environment searches found no exact decision accepting agent-created convenience
updates.","FILTER.md":"No complete match. Effects are durable convenience state from successful ordinary creates, with
no sub-second human action prerequisite."} Exact cited cover: [] Original stronger consequence and missing premise
remain preserved: "Wrong future container-place default and eviction of GUI recent setups; no security or lost
user-owned work claimed. The history explicitly identifies overlap with externally supplied g050/l1 rather than claiming
an independently novel storage cause."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g020 record](records/review-through-00de0b6c/groups/g020.json). Record ID: `l5`.

### g050/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (finding, open):

# Agent environment creates alter the user's container default and recent setups

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435, integrating the history API introduced at
52cc5d7250d262f93b730d91a07c240d33c2ea5f.

## TLDR

A container created at an agent's request changes which container host the GUI preselects next. Agent-created
environments also age out the user's saved recent setups, even though agent creates are not meant to teach GUI defaults
or suggestions.

## Details

The assigned storage foundation accepts captured environment_target independently of explicit_selection.
store/environment_history.rs:49–54 accepts a target with no structured GUI selection; 60–75 inserts a shared history
admission marker, 82–88 overwrites preferences.last_container_host for every fresh container admission, and 93–99 evicts
older entries from the shared bounded window. That is correct for an ordinary GUI command launch, which has no
structured selection but does have explicit GUI place intent. It lacks a separate provenance input distinguishing that
case from an agent launch. NULL selection cannot make the distinction. The reader at 111–114 hides NULL-selection rows
from recent setups, but the markers still age the same window.

The concrete agent route is added by 190d844e58c96cf197aa838d0b0eea8062f48435: agent_requests.rs:4541–4548 passes the
agent request and selected place to start_session_environment. provisioning/tensorlake_add.rs:357–361 unconditionally
attaches captured environment_target, retains the request at 465–469, and finish_session_launch at 1007–1013 sends the
same request through create_in_session_sandbox. sessions.rs:1243–1252 preserves CreateOrigin::Agent; 1848–1863
propagates that origin into both ordinary and managed-checkout creation. Its ordinary path captures environment_target
at 1893–1894. At 2055–2060 explicit_selection is withheld for agents, but record_created_history (2174 onward) passes
the environment target independently to record_create_history_with_environment_if_current; the closure permits
session-owned entries when an environment target is present. No origin bit reaches store/environment_history::record to
suppress either the place write or the shared marker. A successful current agent create therefore executes the shown
SQL. Fresh disposable hosts ensure this is a novel admission, not a replay.

The visible default consumer is environment_ui.rs:716–732 (endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db):
last_container_host chooses the default usable place, and the New container button at 781–792 applies that place. The
original patch's environment_history_raw_creates_age_the_shared_window test in store.rs:10548 onward explicitly
establishes the other mechanism: NULL-selection creates at distinct hosts can evict all structured environment recents.
It tests intended raw-GUI semantics, not agent provenance. The endpoint retains the same storage implementation and
integration.

Contract at the integrating revision, SPEC.md:5158–5163: 'Only what the user explicitly selects in the GUI may affect
the GUI’s future defaults and suggestions' and 'anything an agent creates do not qualify, because none of them is a
choice the user made in the GUI.' The paragraph explicitly names recent setups; this is not solely an interpretation
about the newly added container default. SPEC.md:4330–4335 requires a user's Allow on the agent's environment card, but
does not make the agent's request a user's GUI launch selection; agent-origin exclusion is still deliberately enforced
for selection at sessions.rs:2055. Applying the general successful-environment marker to every origin bypasses that
exclusion for default-place memory and recent-window eviction.

Keep GUI raw commands eligible to remember place and age the user's history, while excluding agent-origin creates from
GUI convenience bookkeeping (or supply a separate trusted user-choice provenance bit). Do not infer GUI authority from
target presence or from the per-launch Allow. Storage API owner: g050; caller/provenance owner for the foundational
integration: g024; the new agent consumer is the agent-request interface exemplified by parent-supplied g020.

Suggested bucket: other — established wrong preselection and suggestion eviction, with no established execution,
credential or user-work loss from these effects. Could-be-highest: no established route; the user still sees the place
when launching, and this finding does not assert g020/l1's separate pending-confirmation endpoint switch. Confidence:
confirmed by source and contract analysis, not execution. proposed_drop: false. possible_cover: none known for these
default/history effects; g020/l1 is explicitly distinct. Caveats: needs an approved successful agent environment launch
on a current connection; evicting the entire bounded recent window needs enough such creates. The code's current-claim
guard works, but current connection authority is not GUI-choice provenance.

Returned proposal (claims-031, event 8): **retain-verified**, suggested bucket **other**.

A successful approved current agent environment launch supplies environment_target but no explicit GUI selection. The
projection still changes container-place memory and ages the shared recent window. This contradicts the explicit
agent-origin exclusion for GUI suggestions.

Original stronger consequence: The lead proposed changed GUI place preselection; the completed finding adds
shared-window suggestion eviction. It never established a security or user-work-loss route. The obsolete-completion
suspicion under g049/l1 and changed-endpoint approval under parent-supplied g020/l1 remain separate roots.

Unresolved premise or limitation: No missing source premise for these conditional effects. Actual occurrence/frequency
is unmeasured. Requires approved successful agent environment creation on a current connection; evicting the entire
100-entry window requires enough fresh creates. No execution, credential exposure or loss of user-owned work follows
from the established convenience effects alone.

Proposed basis: Record one other-priority finding for missing GUI-choice provenance in environment convenience
bookkeeping. Preserve raw GUI command launches as eligible; target presence or an agent card's Allow is insufficient
provenance. Cite g049/l1's latest synthesis as corroboration without merging its independently checked replay concern.

Existing parent disposition (event 8): duplicate. Parent read complete claims-031 g050/l1 and claims-010 g020/l5. Both
establish the same successful current agent environment create reaching record_created_history with environment_target
but explicit_selection=None; store/environment_history.rs:49-99 still inserts an admission and remembers the container
place, while:111-115 hides only the reusable-selection row. This changes GUI defaults and ages its bounded100-entry
window despite SPEC.md:5122-5127's explicit agent-origin exclusion. g020/l5 already retains the defect at other
priority. g050 independently checks introducing52cc5d72 storage,190d844 caller and tip place-default consumer, plus
actual fixture assertions for NULL-selection eviction. Preserve all these source references,
successful-approved-current-connection premise, repeated-create threshold for broad eviction and no observed
runtime/security/work-loss claim. Do not merge the distinct stale-completion authority concern g049/l1 or the highest
physical-place approval route. No coverage/filter acceptance or consequence lowering is inferred.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g050 record](records/review-through-00de0b6c/groups/g050.json). Record ID: `l1`.
