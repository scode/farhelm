# Attention dismissal membership scope unresolved

Reviewed commit: b30f9f6e6c429167404abe09111fe1c19b21bf53

## TLDR

Dismissing optional updates also suppresses later reminders for hosts currently shown as problems. Whether dismissal
should include those hidden offers remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-ui/src/list/view.rs:4234 at b30f9f6e6c429167404abe09111fe1c19b21bf53`.

Source assessment (read with the later corrections below) (review record g079/l2, event 10):

Reviewed commit: b30f9f6e6c429167404abe09111fe1c19b21bf53; behavior also present at pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b list/view.rs:4640-4644. Synthesis revision supersedes the earlier unqualified
claim of a confirmed specification violation. TLDR: Dismissing visible optional updates also remembers update-eligible
hosts currently shown as problems. When their problem clears, their optional reminder can remain hidden even though no
Update row for them was visible at dismissal. That source behavior is confirmed; whether it violates the intended
dismissal contract is unverified after investigation because the specifications leave this transition ambiguous.
Mechanism: attention_list.rs:63-70 builds the full offer set and computes one dismissal-valid flag; :90-99 gives
problems precedence and later suppresses optional Update rows. list/view.rs:4233-4237 captures full host_updates(), and
hosts/updates.rs:79-99 stores all supplied optional/required host IDs plus build, invalidating only for a new offer ID
or changed build. A and B both offer optional updates; B has an unseen warning and renders as Problem while A renders as
Update. Dismiss, then open Hosts to acknowledge B (hosts.rs:1865-1870 and :5428-5430), then return to Sessions: B
remains update-eligible but its reminder is suppressed. Problem rows themselves were never dismissed, and rail
badges/manual Update remain. Exact contract comparison: SPEC.md:2082-2085 says 'A close control dismisses only the
optional Update rows' and that problems/working hosts cannot be dismissed, with renewal when an outside host gains an
update. This can mean only the currently rendered optional Update rows enter the dismissal, or only those row kinds are
hidden when the saved set is applied. SPEC_impl.md:7052-7057 says the existing page-local UpdateDismissal 'remains',
captures 'update host ids and the helm build', and 'applies only to optional Update rows'; required, Problem and Working
rows remain visible. This explicitly preserves the representation and kind-based visibility rule, but does not
explicitly say that IDs masked by Problem classification enter the set, nor what should happen when a known offer
changes from Problem to Update. By contrast, Update all explicitly uses exactly displayed Update rows
(SPEC.md:2077-2079; SPEC_impl.md:7034-7035); that exact-set language is absent from dismissal. Predecessor
SPEC.md:2054-2059 and SPEC_impl.md:7030-7036 described a card naming its optional update hosts, so retaining its
dismissal model does not by itself settle the newly introduced hidden-offer case. Reconciliation with g080 l2: agree on
source behavior, reachable combination, and lack of highest impact; neither g080's definite acceptance nor my earlier
definite violation follows unambiguously from the text. Retain as an explicit contract/behavior ambiguity for parent
handling, not as a confirmed functional bug and not silently dropped. Missing premise: whether dismissal means all
currently known update offers, including ones masked by problems, or only optional Update rows actually shown. Readable
source/spec comparison cannot establish that product choice; an authoritative clarification naming this transition would
settle it. If full-offer dismissal is intended, document that a later-cleared Problem may reveal no reminder; if
visible-row dismissal is intended, distinguish known-offer baseline from actually dismissed row IDs or otherwise handle
transition invalidation. Suggested bucket: other (optional reminder visibility/contract clarity only). Confidence:
source behavior confirmed; claimed violation unverified after investigation. proposed_drop: none. possible_cover:
SPEC_impl.md:7052-7057 is a plausible acceptance basis but is not explicit about hidden-offer membership; no queue or
wider coverage search performed. Caveats: no runtime reproduction, no security/process/data-loss route, and no
rendered-answer timing or wrong-host authorization defect. l1/c3/c4 remain withdrawn on their original narrow source
refutations.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g079/l2`; supporting records
`g008/g008-7`, `g080/l2`, `g081/l2`. The retained private log preserves earlier wording and its corrections. This record
does not claim runtime reproduction.
