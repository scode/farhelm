# Remote version display retains format characters

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

Remote version text bypasses the promised presentation cleaning and can retain invisible formatting characters; no
execution sink or harmful ordinary producer was established.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/shell.rs:86-91 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g052/l4, event 20):

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c. TLDR: a Tensorlake builder can put invisible or
direction-changing characters in the agent versions shown in Settings, where Farhelm displays them invisibly instead of
making them visible. Exact introduced seam is ShellOutput::last_line at shell.rs:86-91: last nonempty stdout line is
trimmed and copied without presentation escaping. Successful SSH stdout at :183-185 is returned unchanged;
build.rs:756-776 reads that line for Claude/Codex, :795-802 persists it, and :519-526 copies it into SnapshotView.
build_script.rs:116-131 agent() runs the remote bare agent command with --version, so the producer is remote program
output. The corresponding UI caller tensorlake/recipes.rs:162-170 appends raw versions into a status string; :346-351
interpolates the string as ordinary DOM text without display_peer/PeerText visible escaping or direction isolation. A
returned line such as a version with embedded U+202E or U+200B remains present after trim and reaches the status span.
This does not require a production GUI input control to manufacture the bytes: the host-controlled command output is the
declared trust-boundary source. SPEC.md Security at :3955-3964 treats remote/agent output as untrusted and explicitly
requires host-supplied GUI text outside a terminal to escape invisible/direction-changing characters wherever it
appears. HTML/JSON escaping only prevents markup parsing, not these presentation effects. Snapshot versions are outside
a terminal. Suggested bucket other, for misleading non-destructive status display; no security, destructive confirmation
or user-work-loss route established. Confidence confirmed by source dataflow, not a runtime measurement. Temporal: diff
of assigned shell.rs from reviewed commit to pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b contains only
script_argv's access.options to access.oneshot_options change. Pinned-tip UI recipes.rs:162-170 still appends raw
versions and its status-line renderer still directly interpolates them; no repair. Suggested fix: apply shared peer-text
visible escaping and direction isolation at the GUI consumer; keep raw stored values if needed. Verification when
authorized can feed a version with embedded format controls and assert visible escape text in the snapshot row.
Source-only pass ran no tests or GUI experiments. proposed_drop false. possible_cover unknown; parent handles
queue/plan/filter and other owner coverage. Distinct from known l1/l3 issues and l2's refuted error-rendering concern.
New discovery confidence kind: verified. No missing premise needed for this non-security display-contract finding;
attacker reachability beyond the declared remote-output boundary is not being asserted. Final consumer-boundary check:
original API tensorlake_recipes.rs:15-25 deserializes RecipesView directly and returns body.recipes without
normalization; proto SnapshotView at tensorlake/recipes.rs:287-296 derives ordinary Deserialize with Option<String>
version fields and no custom deserializer. Helm recipes.rs:525-558 builds and serializes the view directly. Thus no
intervening serialization/client layer escapes the version bytes. Pinned-tip recipe renderer at :346-351 was also reread
and still interpolates raw status text. Discovery pass is complete with l4 as the sole novel verified finding; no novel
unverified-after-investigation finding.

Later qualification (review record g052/l4, event 22):

Reviewed 744ea4122d033ef616454fe592ecb2173593736c. Corrects the earlier successful-stdout return reference from :183-185
to numbered shell.rs:177-179; last_line remains :86-91. No NEW discovery this pass. TLDR remains
invisible/direction-changing characters in Tensorlake builder version stdout being displayed unescaped in Settings.
Successful stdout is decoded unchanged; last_line trims and copies, build.rs:756-776 and :795-802 persist the version,
view :519-526 copies it, and UI tensorlake/recipes.rs:162-170 and :346-351 append/interpolate it as ordinary DOM text.
The direct JSON path and plain Option<String> fields have no normalizer. SPEC.md Security :3955-3964 requires visible
escaping for remote output outside terminals. A remote --version line with embedded U+202E/U+200B reaches this existing
source boundary; no security or destructive action effect is claimed. Bucket other, confidence verified by source
dataflow; no GUI experiment. Survives pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b as checked in discovery
pass 1. proposed_drop false; possible_cover unknown, parent handles coverage. Apply shared peer-text visible
escaping/direction isolation at display. Prior l4 investigation and temporal checks remain intact; this event corrects
line coordinates only.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g052/l4`; supporting records
`g065/g052-l4`, `g070/p3l1`. The retained private log preserves earlier wording and its corrections. This record does
not claim runtime reproduction.
