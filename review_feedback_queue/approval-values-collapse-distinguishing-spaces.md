# Approval values collapse distinguishing spaces

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

Different folders or template arguments can look identical on an approval card because repeated spaces collapse. The
approved action retains the original spaces. Harmful work in a different folder or a security-relevant argument change
has not been demonstrated.

## Details

Ordinary approval values render in `dd/span` elements through `display_peer`
(`crates/farhelm-ui/src/approvals.rs:520–523`). That function preserves ordinary spaces (`peer.rs:159–182`), while the
value-row CSS does not preserve whitespace (`assets/app.css:7682–7696`; `.peer-value` supplies bidi isolation). Thus
`/work/project  copy` and `/work/project copy` can have the same displayed spelling. Full command blocks use pre-wrap;
ordinary value rows do not.

Launch folders (`approvals.rs:768`), template custom permission flags (`1103–1104`) and template folder values
(`1136–1139`) use those ordinary rows. Folder bytes survive resolution and become the launch directory
(`farhelm-helm/src/agent_launch.rs:193–195`, `agent_requests.rs:1773`; supervisor `service/core.rs:7940–7946`). Custom
permission vectors preserve quoted spaces (`farhelm-proto/src/launch.rs:622–656`) and are copied into argv
(`farhelm-helm/src/launches.rs:457–467`). Template-write cards have no separate compiled command that restores the lost
distinction. Launch command blocks do preserve spaces, so their flag display has an additional faithful presentation;
the folder rows do not.

Preserve meaningful whitespace in these approval values. Scrolling cannot recover bytes already collapsed by layout. No
runtime verification is requested or authorized by this review; the user stopped reproduction work.

Suggested bucket: **provisional highest**, conditional on harmful wrong-folder work or security-relevant custom
arguments. Confidence: **confirmed static accepted-input/rendering mismatch; likely conditional consequence**. No
browser rendering, harmful alternate target or executed action was observed. Approval is still required, and this
finding does not establish a server-side target substitution. The severity boundary remains for triage.

Possible cover: `removal-destination-collapses-distinguishing-spaces.md` concerns removal/refusal paragraphs with
different effects, not launch/template authorization. Installation-ID and file-disclosure decisions
(`TRIAGE_OUTCOMES.md:13688–13721`) concern other renderers. `SPEC.md:2564–2586` and `SPEC_impl.md:5626–5664` require the
card to show the folder and full resulting template. No exact Planned, BUGS, ledger or filter coverage was found.
Proposed drop: none.

Provenance: persistent reviewer `A21-card-meaning/A21-F1`; parent checked rendering, retained folder/argument paths and
coverage at the reviewed revision. Source inspection only.
