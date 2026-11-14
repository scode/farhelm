# Agent guide explicitly qualifies key and wait advice for environment launches

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Investigated whether new environment launches were left under the old unconditional guide advice
to use keys and ten-minute waits. The current full rendered manual explicitly supplies the needed exception.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g018/f4`.

### g018/f4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 16 (candidate, open):

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435. TLDR: Investigated whether new environment launches were left
under the old unconditional guide advice to use keys and ten-minute waits. The current full rendered manual explicitly
supplies the needed exception. Details: read render in full through line 294 at the new behavior revision, rather than
relying on the assigned 12c79f6 instruction snippet or selected-path log summary. Ordinary advice remains at 235–238 and
retry advice at 282–287, but the environment-specific paragraph at 258–267 names recipe creates, recipe templates and
environment clones. Exact text at 263–266: 'They take no --host or --idempotency-key; the folder is the new home, or
--gh clones there. Building a new environment can take many minutes after the user allows it: run these with a tool
timeout of at least 40 minutes, or in the background, and if the command reports an unknown outcome, check farhelm agent
sessions before trying again.' That is an explicit specific qualification of the ordinary advice, not a missing
exception. The paragraph is concatenated into the same render output; generated verb text does not omit it. A diff of
this file from this revision through pinned endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db is empty. Suggested
bucket: other if the original instruction defect were present; no actual defect established. Confidence: confirmed
source refutation. proposed_drop: explicit current render text quoted above, not minor impact or production correctness.
possible_cover: none; parent supplied no matching interface finding. Could-be-highest: no; original suspected
consequence was unnecessary refusal or premature unknown outcome, no established security/work-loss route. Source-only;
no CLI run, test or reproduction. This closes f4. Earlier source basis was from the prior 12c79f6 revision, and its
missing-exception premise is now resolved by checking the correct 190d844 snapshot. Later synthesis turn 5: reread the
full local history and exact supplied g020/l7 event23 and g005/f2 event7, then rechecked this guide paragraph at
190d844e lines 258–267. The guide explicitly says at least 40 minutes or background execution and to inspect sessions
after unknown outcome; it promises no ordering between relay and helm deadlines. Its exception refutes only f4's
missing-key/wait-guidance concern. Independently confirmed g020/l7's caller-comment defect at agent_requests.rs
4550–4554 against relay queue-relative and helm post-registration timer starts; that finding stands with its original
owner and is corroborated under local candidate f5. g005/f2 separately concerns receipt-relative versus post-Allow
effective-span wording. Neither is erased by correct guide behavior, nor incorporated into f4 as a new consequence.
Existing f4 proposed-drop basis remains the explicit guide text alone; no parent drop is presumed. No unresolved premise
remains for f4 and no runtime behavior was observed.

Returned proposal (claims-009, event 11): **refuted**, suggested bucket **other**.

The full guide at the environment-launch revision explicitly excludes idempotency keys and requires a forty-minute tool
budget or background execution for new environments.

Original stronger consequence: An agent might add a forbidden key or use only the ordinary tool wait, causing needless
refusal or premature unknown outcome. The history withdrew this after reading the complete new render text. Duplicate
environment creation, security harm and user-work loss were not established.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Source refutation by explicit behavior-specific guide text, not by low impact or correct product
implementation.

Existing parent disposition (event 11): excluded. Parent explicitly accepted named refuted proposal from claims-009
after inspecting it. The full guide at the environment-launch revision explicitly excludes idempotency keys and requires
a forty-minute tool budget or background execution for new environments. Source refutation by explicit behavior-specific
guide text, not by low impact or correct product implementation. Checked source/results:
["190d844e58c96cf197aa838d0b0eea8062f48435, crates/farhelm/src/agent_instructions.rs:235-238 retains ordinary
ten-minute/key advice. Lines 258-267 in the same concatenated render output identify recipe creates, recipe templates
and environment clones and provide the specific exception.","Lines 263-266 say these take no --host or
--idempotency-key, require at least forty minutes or background execution, and require listing sessions before retrying
an unknown outcome. This qualifies the general retry/key guidance at 282-287.","git diff of agent_instructions.rs from
190d844e to 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db is empty. The earlier lead used surrounding text from 12c79f61
before the new paragraph existed. No CLI execution or runtime observation."] Current coverage/results: ["SPEC.md Agents
and environments explicitly states key refusal, the combined approval/build budget and unknown-outcome behavior.
SPEC_impl.md Agent requests and environments explains the relay's extended budget and that expiry leaves the task
running. These corroborate the guide exception, rather than cover a real erroneous guide.","TODO.md Planned is empty;
BUGS.md's SSH-helper hang is unrelated.","plans/queue searched for agent guide/instruction timeout/key requirements. Its
unlanded index is empty; no pending correction covers this claim.","Queue index/files and active TRIAGE_OUTCOMES.md
searched for agent instructions, manual keys, environment waits and idempotency guidance. No exact existing item or
decision found. Archive not read.","FILTER.md read. No filter used; source wording resolves the missing-exception
premise."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "An agent might add
a forbidden key or use only the ordinary tool wait, causing needless refusal or premature unknown outcome. The history
withdrew this after reading the complete new render text. Duplicate environment creation, security harm and user-work
loss were not established."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g018 record](records/review-through-00de0b6c/groups/g018.json). Record ID: `f4`.
