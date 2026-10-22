# Different folders can have identical approval text

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

A launch or template approval can show the same text for two different folders. An invisible character in one path is
displayed like literal escape notation in the other, potentially concealing which folder the user is approving.
Meaningful alternate folders and harmful reliance have not been observed.

## Details

Found by persistent reviewer `A21-card-meaning`, finding `A21-F4`, and checked against source by the recording agent.
Suggested bucket: provisional highest, conditional on harmful wrong-folder work or security-relevant configuration.
Confidence: confirmed static display collision and raw-path admission; likely user impact with the alternate-directory
and reliance premises unverified. No browser, filesystem or execution reproduction was performed.

Launch folders (`crates/farhelm-ui/src/approvals.rs:768`) and template folders (`1136-1139`) use `CardRow::Value`. That
renderer calls `display_peer` (`520-523`), which replaces unsafe characters with `<U+XXXX>` but preserves a literal `<`
(`crates/farhelm-ui/src/peer.rs:159-180`). A path containing U+200B and one containing the eight literal ASCII
characters `<U+200B>` consequently have the same spelling. The shared unsafe list includes U+200B
(`crates/farhelm-proto/src/text.rs:57-79`). This occurs before layout; preserving spaces alone cannot distinguish them.

Template application keeps the raw destination (`crates/farhelm-proto/src/launcher.rs:859-860`), and agent launch
resolution retains the folder (`crates/farhelm-helm/src/agent_launch.rs:191-194`). Template validation excludes folder
fields from its invisible-character prohibition (`launcher.rs:444-502`). The supervisor accepts an existing absolute
directory without rejecting U+200B (`crates/farhelm-supervisor/src/service/core.rs:3532-3576`). A requesting agent can
therefore name such a destination on another host, subject to ordinary launch approval and admission. Useful alternate
directories are a prerequisite, not evidence already established. The visible escape does signal unusual text.

The identity formatter avoids this class of collision by escaping literal `<` (`peer.rs:204-232`). A correction should
give actionable folder disclosures a collision-free spelling while retaining the raw path for execution. The separate
`approval-values-collapse-distinguishing-spaces.md` concerns CSS space collapse and would not fix this collision.

Possible cover: SPEC.md:3198-3202 requires visible escaping and explicitly requires distinct identity labels; it does
not explicitly accept lossy approval folders. SPEC.md:2573-2579 and SPEC_impl.md:5626-5664 bind consent to the displayed
action. Ledger `file-disclosure-misses-invisible-path-characters.md` (`13704-13720`) fixed literal-escape collisions in
file disclosures, while `template-editor-conceals-peer-model-and-folder.md` (`13722-13736`) protects editor bytes;
neither covers approval cards. The repository-label discard (`17500-17512`) relies on validation rejecting malformed
repository names, unlike existing folder paths. Identity-only fixes at `13689-13702` and `17514-17532` likewise do not
cover these rows. No matching Planned item, BUGS entry or queued approval-path finding was found. The filters exclude
harmful wrong-target and trust-boundary consequences. Proposed drop: none.
