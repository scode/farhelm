# Archive packer changes an accepted explicit payload directory

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: distributed Rust test staging fails for a valid operator-supplied payload directory
whose final path component ends in whitespace, because the packer removes that part of its name. Contract and route:
rust-tests.ts:46-49 explicitly supports an operator-supplied matching payload directory.
scripts/build-podman-test-payloads.sh:26 reads FARHELM_HELM_PAYLOAD_DIR (or the private staged input), :57-64 accepts an
absolute existing directory with the required files, rejecting line breaks but not trailing spaces or tabs, and :68
prints the exact path followed by one newline. The new caller at rust-tests.ts:62 applies String.trim() to the whole
output; :63 passes the changed path to tar -C. For an existing absolute directory ending in a space and no sibling at
the stripped path, the producer validates successfully and tar is then directed to a nonexistent directory, aborting
staging. This is a source-established conditional failure, not an execution observation. Preserve the producer's path
and remove only its record-ending newline, or explicitly reject an unsupported name before promising it was accepted.
Attribution: the parent revision's pack():47-56 has no payload producer/output parsing; this exact call is added by
04d85f0. git log over the entire pinned range for rust-tests.ts lists only 04d85f0 and d751ff1; the pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:62-63 still has the same trim() and tar call. SPEC_impl.md, Testing, does not
waive the tool's explicit directory contract; its single-user and single-run assumptions do not explain changing an
accepted pathname. Sugg

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g147/l2`.

### g147/l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc. TLDR: distributed Rust test staging fails for a valid
operator-supplied payload directory whose final path component ends in whitespace, because the packer removes that part
of its name. Contract and route: rust-tests.ts:46-49 explicitly supports an operator-supplied matching payload
directory. scripts/build-podman-test-payloads.sh:26 reads FARHELM_HELM_PAYLOAD_DIR (or the private staged input), :57-64
accepts an absolute existing directory with the required files, rejecting line breaks but not trailing spaces or tabs,
and :68 prints the exact path followed by one newline. The new caller at rust-tests.ts:62 applies String.trim() to the
whole output; :63 passes the changed path to tar -C. For an existing absolute directory ending in a space and no sibling
at the stripped path, the producer validates successfully and tar is then directed to a nonexistent directory, aborting
staging. This is a source-established conditional failure, not an execution observation. Preserve the producer's path
and remove only its record-ending newline, or explicitly reject an unsupported name before promising it was accepted.
Attribution: the parent revision's pack():47-56 has no payload producer/output parsing; this exact call is added by
04d85f0. git log over the entire pinned range for rust-tests.ts lists only 04d85f0 and d751ff1; the pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:62-63 still has the same trim() and tar call. SPEC_impl.md, Testing, does not
waive the tool's explicit directory contract; its single-user and single-run assumptions do not explain changing an
accepted pathname. Suggested bucket: other (maintainer tooling; edge-case path handling). Confidence: confirmed from
source, with the named input shape as the trigger; no affected operator directory was inspected. proposed_drop: false.
possible_cover: none known from the permitted interface records; no prohibited coverage search. Could-be-highest: no;
failure occurs during private staging, with no established security or user-work-loss route. Caveats: normal generated
directories end in the architecture name, so the default path does not trigger it; this is limited to an explicit
override whose directory itself ends in trim-eligible whitespace. No runtime test or reproduction was performed. Lead l2
is resolved to this finding under its original ID.

Returned proposal (claims-094, event 13): **retain-verified**, suggested bucket **other**.

The producer accepts an explicit absolute existing directory ending in whitespace, while the new packer strips that
whitespace before passing its path to tar. When the stripped sibling does not exist, staging fails.

Original stronger consequence: Distributed Rust test staging fails for a valid explicit payload directory ending in
trim-eligible whitespace. Default generated directories end in the architecture name and do not trigger it. No security
or user-work-loss consequence was asserted.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain as source-verified maintainer-tool path handling, with the exact input shape and no runtime
claim.

Existing parent disposition (event 13): verified. Parent explicitly accepted named retain-verified proposal from
claims-094 after inspecting it. The producer accepts an explicit absolute existing directory ending in whitespace, while
the new packer strips that whitespace before passing its path to tar. When the stripped sibling does not exist, staging
fails. Retain as source-verified maintainer-tool path handling, with the exact input shape and no runtime claim. Checked
source/results: ["04d85f0c98a9b635709d00b056719a0baa7fd3fc:scripts/build-podman-test-payloads.sh:26,55-68 preserves
explicit directory spelling, rejects CR/LF but not final spaces or tabs, verifies required files and prints exactly the
path plus newline.","Same revision scripts/tensorlake/rust-tests.ts:46-49 explicitly supports operator-supplied matching
directories; :62 trims the whole output and :63 uses the changed value as tar's -C argument. The parent pack():47-56 had
no such producer parsing.","Pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:rust-tests.ts:62-63 retains the same
trim/tar call. Range log lists only 04d85f0 and d751ff1 for this file. Conditional source conclusion only; no affected
directory inspected or command reproduced."] Current coverage/results: ["SPEC_impl.md Testing:8163-8230
single-user/single-command assumptions do not waive path preservation; provisioning :8038-8052 accepts operator-staged
directories. SPEC.md searches found no tooling path grammar exclusion.","TODO.md Planned empty; BUGS.md unrelated;
active ledger and plans searches found no exact final-whitespace payload-directory item. INDEX.md and matching queue
files were checked: rust-sandbox-staging-misquotes-checkout-path.md concerns checkout path shell quoting,
rust-sandbox-pack-ignores-cargo-target-dir.md concerns build output selection, catalog-unencoded-file-url.md concerns
browser module URL conversion. None covers this producer-output parsing defect.","FILTER.md rare-safe-failure rule does
not fully match: retrying the same accepted override repeats the failure; it needs path/source repair. No other filter
applies."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Distributed Rust
test staging fails for a valid explicit payload directory ending in trim-eligible whitespace. Default generated
directories end in the architecture name and do not trigger it. No security or user-work-loss consequence was
asserted."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g147 record](records/review-through-00de0b6c/groups/g147.json). Record ID: `l2`.
