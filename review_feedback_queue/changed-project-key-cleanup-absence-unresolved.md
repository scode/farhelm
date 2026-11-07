# Changed project key cleanup absence unresolved

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

After an account key changes projects, cleanup may confuse inaccessible resources with absent ones. The provider's
foreign-ID response behavior remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Recording judgment: The recorder assigns other to the established project-identity/response interpretation gap.
Host-removal false success and resources remaining billable depend on the still-missing foreign-project API response
premise; the contributing owner's conditional high suggestion is preserved, rather than presented as an observed
failure.

Source location:
`crates/farhelm-helm/src/tensorlake/build.rs:1230-1237, 1290-1294 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g068/l4, event 10):

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c. TLDR: if a key for a different Tensorlake account is saved,
deleting or rebuilding recipes can forget snapshots still billing in the previous account, depending on Tensorlake's
cross-account response. Mechanism: RecipeState stores builder/snapshot IDs without account identity; clean_for_delete
reads the latest configured key (1290-1294), and sweep_builder/delete_retired treat NotFound as confirmation of deletion
and remove IDs (1168-1169,1195-1207,1230-1237). Config::saved accepts any replacement visible-ASCII key without checking
resource ownership (config.rs:48-75), and save_tensorlake_config persists it without consulting recipes (131-157). With
an account-scoped API returning 404 for IDs outside the new account, cleanup removes old records while those resources
survive. SPEC.md Settings and SPEC_impl.md Tensorlake account explicitly support replacement and do not bind resource
ownership; Topology promises unfinished cleanup remains tracked. Suggested bucket: high, conditional on verifying the
cross-account 404 premise, for indefinitely billed cloud storage after an ordinary settings change. This revises the
earlier other recommendation: priority follows the consequence, while uncertainty belongs in confidence rather than
artificially lowering priority. No established security or user-work-loss route to highest. Confidence: likely
conditional mechanism, unverified after investigation. Actual checks: inspected original and pinned-tip config save
source, original RecipeState and cleanup paths, HTTP status classification (api.rs:264-271), both governing SPEC
sections and assigned-file diff through pinned tip. Attribution and cleanup remain unchanged; config remains
unrestricted. Missing premise: production Tensorlake's response for another account's resource IDs, 404 versus 401/403.
Readable local sources do not pin that behavior; network experiments are prohibited. Verify using vendor documentation
or a controlled two-account API test in a separately authorized task. A 403 would preserve records and refute the
forgetting route. Fix direction if confirmed: bind resources to account identity and reject unsafe switching, or refuse
to treat absence in an unverified account as deletion. proposed_drop: false. possible_cover: unknown, parent handles
coverage. Caveat: changing credentials within the same account must stay supported.

Later qualification (review record g068/l4, event 14):

Delta to complete l4 initial record: authorized read-only primary-source search found
https://github.com/tensorlakeai/opencode-tensorlake-plugin/blob/main/README.md, Requirements/Log in, explicitly
describes project API keys selecting organization/project and says only holders of a project key can see that project's
resources. https://github.com/tensorlakeai/tensorlake/blob/main/README.md describes per-project contexts/tokens. This
supports distinguishing a different project even within one account, so the original wording 'different account' is
unnecessarily narrow. It does not establish whether foreign sandbox/snapshot IDs return 404 or 403. Continuing primary
vendor docs/source inspection only, no authenticated API/resource request, runtime or network experiment. Prior
conditional deletion mechanism, high consequence if verified, proposed_drop false and possible_cover unknown retained.
Unverified after investigation remains; exact foreign-ID response premise is still missing.

Later qualification (review record g068/l4, event 18):

Delta to initial l4: primary https://docs.tensorlake.ai/api-reference/v2/sandboxes/delete.md documents 403 for lack of
resource permission and 404 for sandbox not found, but does not say which response an otherwise-valid project-B key
receives for a real project-A resource ID. Therefore a generic documented 404 is NOT proof of the hypothesis; generic
403 is NOT exact refutation either. https://docs.tensorlake.ai/platform/authentication.md confirms keys exist only
within their project and select that scope; current SDK source
https://raw.githubusercontent.com/tensorlakeai/tensorlake/main/src/tensorlake/sandbox/client.py for_cloud:432-459 says
ingress selects the authorized org/project. Its delete/get simply map 404 to not-found; that is client classification,
not the server authorization decision. Remaining check: snapshot DELETE/get-specific response docs and accessible public
server source, if any. No actual resource API call or runtime experiment. Keep exact missing 404-versus-403 premise,
conditional mechanism, proposed_drop false and possible_cover unknown; no highest consequence claimed.

Later qualification (review record g068/l4, event 22):

Reviewed commit 744ea4122d033ef616454fe592ecb2173593736c; cleanup/account attribution unchanged at pinned
8a8c9579e04299410652f0e4681975a7198dfc7b per initial source checks. TLDR conditional: replacing the API key with one for
a different project may cause cleanup to forget still-billed old-project resources IF foreign IDs return 404. Source
mechanism remains: cleanup fetches latest key, assumes NotFound means deletion and removes records without
account/project identity. Synthesis correction: different project is sufficient as a scope change, not necessarily
different account. Primary evidence checked: https://docs.tensorlake.ai/platform/authentication.md API keys/SDK and CLI
authentication explicitly makes keys project-specific and says the key selects project;
https://docs.tensorlake.ai/platform/access-control.md API Keys gives project-member scope but no foreign-ID status rule.
https://docs.tensorlake.ai/api-reference/v2/sandboxes/get.md and
https://docs.tensorlake.ai/api-reference/v2/sandboxes/delete.md enumerate 403 lack of resource permission and 404
missing sandbox without assigning the existing-foreign-project-ID case.
https://docs.tensorlake.ai/sandboxes/snapshots.md Get Snapshot Details/Delete a Snapshot documents ID paths but no
response-status conditions.
https://raw.githubusercontent.com/tensorlakeai/tensorlake/main/src/tensorlake/sandbox/client.py:432-459 confirms
ingress-selected project; get:1022-1044/delete:1352-1369 map 404 and delete_snapshot:1761-1775 forwards errors, all
client-side classification rather than server authorization. Public Cargo.toml lists SDK/CLI/client bindings, not cloud
authorization server; public openapi.yaml has no /sandboxes or /snapshots routes (exact path search), and
https://github.com/tensorlakeai/indexify was unavailable/404. The guessed docs snapshot API-reference page also 404;
searched only official vendor docs/source, with irrelevant search hits unused. Tool fetch could not parse Markdown, so
public document text was retrieved read-only by curl; no resource API request, credential access, experiment, build,
install or runtime test. Exact missing premise: for an existing project-A sandbox/snapshot ID with a valid project-B
key, does each cleanup-relevant endpoint return 404/empty project-B listing rather than 403? A 403 preserves the record
and refutes that endpoint's forgetting route; project scoping alone and generic 404 listings do not settle it. No
accessible primary evidence found resolving this, and current docs cannot establish historical response behavior. Keep
finding unverified after investigation. Suggested bucket other, revising earlier conditional high: no material billing
amount/common recurrence or severe user consequence is shown; possible residual storage billing alone does not establish
high. Confidence conditional source mechanism only, vendor premise missing; proposed_drop false, possible_cover unknown;
no highest route or independent highest drop. A vendor server rule, cross-project contract or separately authorized
observation could settle the premise; none fabricated here.

Later qualification (review record g068/l4, event 25):

Selected g073:f4 line19 supplied in full this turn; shared missing project identity/foreign-ID status premise, not NEW
discovery. Preserve owner f4's separate c340166d754d3edb3de7fd538c6408948d3536c6 orphan retry path and in-memory
operation record; do not merge away this original persisted recipe cleanup path at
744ea4122d033ef616454fe592ecb2173593736c. Owner recommends high conditionally; this log recommends other until material
severe/common consequence is shown, with dispute retained for parent, not resolved by asserting confidence as severity.
Owner rendered docs exposed no error statuses; this review's raw Markdown OpenAPI snippets exposed generic 403 and 404.
Neither surface specifies valid foreign-project ID behavior. Prior exact missing premise and
unverified-after-investigation state unchanged; no repeated network investigation, proposed_drop false, possible_cover
related root g073 f4, not identical affected path. Same-project key rotation must remain supported.

Later qualification (review record g068/l4, event 34):

Selected supplied g032:g032-l6@19 and g073:f4@21 synthesized; not NEW findings. g032-l6 preserves original reviewed
88bf83e267b5066815448885c61ed4148b851930 host-removal path, including its conditional false-success/forgetting
consequence; its new local evidence confirms config save transaction and TensorlakeBinding lack a project guard, not
provider status semantics. g073 f4 preserves c340166d754d3edb3de7fd538c6408948d3536c6 orphan enumeration/in-memory retry
retirement scope and now recommends other, aligning this log's builder/retired-snapshot cleanup impact judgment. g032
retains conditional high; preserve this remaining severity disagreement for parent rather than asserting all three paths
have identical consequences. All share missing project identity plus valid foreign-project resource response premise; a
different endpoint alone is not novelty. Generic documented 403/404 and differing HTML/Markdown representations do not
establish foreign-ID behavior. Original persisted builder cleanup at 744ea4122d033ef616454fe592ecb2173593736c remains
distinct. No new provider evidence, no repeated fetch/investigation, confidence unverified after investigation,
proposed_drop false, possible_cover related root g073 f4/g032-l6. Same-project key rotation must remain supported. Known
shared host termination deadline g032-f5/g040-l6/g033-f7/g035-d4 is recognized as duplicate if encountered, not
attributed as a new builder root issue.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g068/l4`; supporting records
`g032/g032-l6`, `g032/g068:l4`, `g032/g073:f4`, `g073/f4`, `g065/r7`. The retained private log preserves earlier wording
and its corrections. This record does not claim runtime reproduction.
