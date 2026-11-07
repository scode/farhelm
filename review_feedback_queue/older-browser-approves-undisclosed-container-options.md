# Older browser approves undisclosed container options

Reviewed commit: 91b6dd23cac4856065f360e3969f0a9d98763be1

## TLDR

An already loaded older released browser page can refresh and approve container choices it cannot display, despite the
current renderer's disclosure repair.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location: `crates/farhelm-ui/src/approvals.rs:1325-1341 at 91b6dd23cac4856065f360e3969f0a9d98763be1`.

Source assessment (read with the later corrections below) (review record g077/d7, event 38):

Reviewed commit 91b6dd23cac4856065f360e3969f0a9d98763be1. TLDR: an old released browser page can still approve template
writes without showing their container image/port/token-file settings after the helm updates. Suggested bucket highest;
confidence source-confirmed released-renderer/attended-refresh route, not a live upgrade reproduction or observed secret
exposure. Released v0.35.0 resolves locally to ee6957b5f0aa10bd4d36834f633b122cf2ec4695: proto/launcher.rs:270-278
decodes container, UI approvals.rs:1288-1344 omits it. The patch adds current rows without changing TemplateWrite
kinds/answer capability. Immediate prepatch source 13fedfc072cc0cdcb8cf27d3b9ba789ac0317d50 approvals.rs:342-358
Explicit-refreshes after any answer, including Gone. Its api.rs:5154-5165 fetches/decodes cards, :2358-2427 merely
observes skew on successful replies, and :5296-5355 posts answers without renderer-version binding. Relevant API bodies
are unchanged from the released UI; release/parent reader and skew blobs are identical. reader.rs:636-668 permits
attended reads under skew. Source-derived shipped sequence: old document retains an expired pre-restart card; a current
window supplies GUI presence for fresh requests; denying the stale card yields Gone followed by Explicit refresh; a new
template card is rendered by the old code and can be allowed without these settings. Helm approvals.rs:672-830 gates by
ID/action, not renderer capability. Final disposition correction: parent reports independent
drop05/verdicts-clarification.json DISAGREED with the accepted-skew drop. SPEC_impl.md Version and skew:7310-7316
intentionally keeps explicit operations usable, but does not expressly waive missing sensitive approval disclosure.
Retain that policy as context only; proposed_drop false. This is final drop adjudication, not new discovery, and I
propose no new exclusion basis. A fix would bind sensitive template approvals to a suitable card kind/capability or
require reload before answering; renderer rows alone cannot patch an already loaded bundle. possible_cover unknown,
parent handles matching. Temporal checks: no changes through 8a8c9579e04299410652f0e4681975a7198dfc7b to helm/proto/UI
approvals, reader or skew; tip API :5176-5190,:5354-5377 and :2394 preserve fetch/answer/stamp behavior. Separate from
d1's current actual-launch omission, d5's current path-escape ambiguity and d4's wording. No deployed credential
contents, actual leak, runtime/browser/process/credential/network-endpoint experiment, build or installation was used.

Recording judgment: Retain highest. drop05 first turn disagrees with outside-highest; second clarification explicitly
DISAGREES with DROP under attended-request version-skew policy. This overrides the owner's proposed accepted-skew
exclusion.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g077/d7`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
