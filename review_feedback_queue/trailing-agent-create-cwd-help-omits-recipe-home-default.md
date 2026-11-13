# Create folder help omits the new recipe launch default

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Create folder help omits the new recipe launch default

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g090/l7`.

### g090/l7

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 17 (finding, open):

# Create folder help omits the new recipe launch default

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435.

## TLDR

The agent create CLI documents --cwd as required unless a template supplies a folder, even for the newly supported recipe launch that supplies the new environment's home automatically. A caller following that requirement is told to provide unnecessary folder input; an ordinary project path is then rejected because a new environment must start at home or in its managed checkout.

## Details

The --cwd help declaration at crates/farhelm/src/main.rs:312–325 says 'Required unless a template sets the folder'. The new --recipe declaration at :334–343 introduces a no-template creation form and says 'Folder is the new home (--gh clones there).' AgentCmd::verb at :703–719 sends destination: None when neither cwd nor gh was provided. In the explicit-recipe consumer, agent_requests.rs:4208–4213 sets edits.target; resolve_placed applies these flags; launcher.rs:960–965 sets destination to Folder("~") when that target is applied without a GitHub destination. Thus a valid recipe and complete launch need no --cwd and no folder template. A supplied non-home path is refused at launcher.rs:970–978 with 'a new environment starts in its home directory'. SPEC.md:4323–4325 explicitly establishes this rule: 'The destination follows the launcher\'s rules for a new environment: a folder is the new environment\'s home, and `--gh` clones the repository inside it.' The field documentation should state this new exception and its folder restriction, alongside ordinary-host behavior.

Attribution caveat from full-range comparison: the identical --cwd wording exists at exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a:314 and at the immediate parent; --gh also already existed there, so the broader claim that --cwd was ever universally required without a folder template was already inaccurate for that old managed-checkout alternative. This finding is specifically the new recipe-launch exception introduced and exposed by the assigned 190d844e CLI patch, not a claim that it introduced the older --gh omission. Before that feature the explicit --recipe form did not exist. The new host help in the same patch is updated for --recipe, while the folder help retains its incompatible requirement. At pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:314 the same folder declaration remains.

Suggested bucket: other, user-facing command documentation correctness. Confidence: confirmed by declarations, exact new verb construction and destination resolution; no generated help output or runtime failure was observed. proposed_drop: false. possible_cover: none in supplied neighbour records; parent owns consolidation. Could-be-highest: no; this is misleading usage guidance and avoidable validation refusal, not a security or loss-of-work route. Ordinary execution and the recipe home default are correct; they establish the documentation contradiction rather than refute it. This closes l7's source and attribution checks, preserving the pre-existing --gh caveat. No tests or reproductions.

Returned proposal (claims-059, event 17): **retain-verified**, suggested bucket **other**.

The new recipe create form needs neither cwd nor a folder template, while cwd help still says one is required. The recipe help partially explains the home destination but does not cure that contradictory requirement.

Original stronger consequence: Misleading usage guidance can cause unnecessary folder input and avoidable validation refusal. No security or user-work-loss route alleged. Preserve the pre-existing --gh caveat; do not attribute that older omission to this patch.

Unresolved premise or limitation: None for the help declaration contradiction. No runtime failure or user reliance was observed; supplying an ordinary non-home path is conditionally refused by source.

Proposed basis: Retain the new recipe-specific help contradiction; correct production home-default behavior demonstrates the documentation defect.

Existing parent disposition (event 17): verified. Parent explicitly accepted named retain-verified proposal from claims-059 after inspecting it. The new recipe create form needs neither cwd nor a folder template, while cwd help still says one is required. The recipe help partially explains the home destination but does not cure that contradictory requirement. Retain the new recipe-specific help contradiction; correct production home-default behavior demonstrates the documentation defect. Checked source/results: ["190d844e58c96cf197aa838d0b0eea8062f48435 main.rs:312–325 says cwd is required unless a template sets folder; :334–343 introduces recipe and says Folder is the new home.","Same revision main.rs:703–719 leaves destination absent without cwd/gh. agent_requests.rs:4208–4213 adds target, :4241–4256 calls resolve_placed; launcher.rs:960–978 supplies home '~' and refuses an explicit non-home folder.","Exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a and immediate parent main.rs:310–335 already contain identical cwd text and --gh. The older gh omission predates the range; the new recipe exception is in range. Pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:314 retains the wording. Source only; generated help was not executed."] Current coverage/results: ["SPEC.md Agents and environments explicitly gives fresh environments their home/managed-checkout destination. SPEC_impl.md Agent requests and environments and Agent launches from the CLI were read; neither accepts contradictory cwd help.","TODO Planned empty; BUGS unrelated. plans/queue searched for cwd/folder help and environment CLI; environment-agent-operations already landed, with no pending help repair. tmux docs/tooling sweeps do not address this recipe documentation.","Full queue INDEX and targeted queue-body/active-ledger searches found no matching cwd help item. No archive read.","FILTER does not match: valid recipe creation is ordinary usage, not rare input or a timing fault."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Misleading usage guidance can cause unnecessary folder input and avoidable validation refusal. No security or user-work-loss route alleged. Preserve the pre-existing --gh caveat; do not attribute that older omission to this patch."; "None for the help declaration contradiction. No runtime failure or user reliance was observed; supplying an ordinary non-home path is conditionally refused by source.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g090 record](records/review-through-00de0b6c/groups/g090.json). Record ID: `l7`.

