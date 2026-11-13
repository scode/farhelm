# Remove the obsolete condition from the migration contract

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The authoritative instructions for moving an existing session to a new terminal still say Restart requires an internal switch that this change removes. A maintainer following that migration paragraph is given a nonexistent prerequisite and an incorrect description of when migration happens.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g006/f1`.

### g006/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 6 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: The authoritative instructions for moving an existing session to a new terminal still say Restart requires an internal switch that this change removes. A maintainer following that migration paragraph is given a nonexistent prerequisite and an incorrect description of when migration happens. Details: SPEC_impl.md:2488-2492 retains 'with the internal switch on, a Restart ... starts the new launch in a terminal host'. Attribution checked against the parent: identical wording was at parent SPEC_impl.md:2504-2508 and was valid there, because parent :2295-2333 defined SupervisorSeams::new_terminals and explicitly allowed Restart to remain in tmux with it off. This commit removes that selector, changes the assigned Every new terminal in a host and Creating agents in hosts paragraphs (:2294-2320), and even removes the same switch condition from Clone/Replace later in this migration paragraph (:2555), but leaves its opening conditional intact. The core.rs diff removes 'let into_host = host_agent || self.new_terminals() == NewTerminals::Host' and makes host settling unconditional (reviewed source :12973-12981); the new-host path therefore has no condition corresponding to the retained instruction. The root contract declares SPEC_impl.md authoritative for implementation choices, so correct nearby paragraphs expose the contradiction rather than refute it. Fix by making this paragraph's Restart/migration statement unconditional as the implementation and the other changed paragraphs already are. Suggested bucket: other, a documentation correctness defect; no shipped runtime divergence or security/data-loss route is established. Confidence: confirmed from parent/reviewed source and patch; no execution. proposed_drop: false. The earlier proposal to drop as minor or resolved by nearby correct statements is withdrawn; preserve its original consequence of misleading maintenance/migration instructions, without upgrading to hypothetical runtime harm. possible_cover: none; no coverage sources searched. Could-be-highest: no established or suspected route from this wording. This closes the synthesis lead under the original ID.

Returned proposal (claims-003, event 6): **needs-parent**, suggested bucket **other**.

The historical documentation defect is verified, but it was removed later inside this same review range. Do not resurrect the reviewer's withdrawn nearby-correct-text refutation: the contradiction really existed at 647f0227. The result enum has no repaired-in-range disposition, so the parent must record that factual resolution explicitly.

Original stronger consequence: Misleading authoritative maintenance/migration instructions give a nonexistent prerequisite for moving an existing session to a host. The original proposal to dismiss as minor/resolved by nearby text was explicitly withdrawn. No higher security or loss-of-work consequence was claimed.

Unresolved premise or limitation: No historical defect premise missing. Parent must choose its recording disposition for a defect repaired later in the pinned range; retaining it as a current unresolved defect would be inaccurate.

Proposed basis: Verified historical defect introduced by removing the selector at 647f0227 and repaired by deleting the migration subsection at 1ed482dd, both within the review range.

Existing parent disposition (event 6): rejected. Parent accepts the factual checks in claims-003 for the historical documentation contradiction: removing the selector at647f0227 left SPEC_impl.md:2488-2492 referring to its internal switch. Nearby unconditional text did not refute that contradiction, and the withdrawn dismissal stays preserved. The later1ed482dd removes that migration subsection; current SPEC_impl.md:2160-2185 makes terminal-host use unconditional. This exact defect was repaired inside the pinned interval, so there is no unresolved current item to queue. This is a later-source repair decision, not an assertion that the historical finding was false; no user-work or security consequence was alleged. Bothspecs/currentPlanned/BUGS/queue/activeoutcomes and unlanded-plan checks found no separate unresolved remainder. All original events and the focused verification remain retained.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g006 record](records/review-through-00de0b6c/groups/g006.json). Record ID: `f1`.

