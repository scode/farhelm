# Attention update accessible name omits target

Reviewed commit: b30f9f6e6c429167404abe09111fe1c19b21bf53

## TLDR

Attention-list Update buttons omit their host and version from the accessible name, leaving identical actions difficult
to distinguish.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/src/hosts/attention_list.rs:295 at b30f9f6e6c429167404abe09111fe1c19b21bf53`.

Source assessment (read with the later corrections below) (review record g079/l9, event 26):

Reviewed commit: b30f9f6e6c429167404abe09111fe1c19b21bf53. TLDR: A screen-reader user focusing an attention-list Update
button hears only Update, without the target host or the version/required-versus-optional explanation available on
hover. Mechanism: attention_list.rs:257-259 computes the identity/status tooltip and host_update_title; :295-301 gives
the action only visible text Update and data-tooltip=update_tooltip, with no aria-label or accessible description. The
tooltip mechanism deliberately hides its popup from assistive technology: assets/tooltip.js:72-76 says nothing
references it and required information belongs on the control; :275-277 creates role=tooltip with aria-hidden=true.
SPEC_impl.md GUI hover-help contract :1286-1291 requires hover information needed by screen readers, specifically a
host's update urgency, to be exposed as accessible descriptions. The existing full Hosts inline Update button already
supplies the intended pattern in hosts.rs:4693-4701 (aria_label Update plus shown_name and aria_describedby), with a
hidden copy of host_update_title at :4726-4728. The new attention surface reuses the hover helper without that
accessibility wiring. The adjacent identity button does expose host and generic Update available/required (:269-280), so
this does not claim the entire row's host/state is inaccessible; it does not give the Update action its own target or
expose the tooltip's version comparison. With multiple update hosts, direct button navigation presents indistinguishable
Update actions. Suggested fix: give each attention action an accessible name including its displayed host and a uniquely
identified hidden description containing the same host_update_title text; do not rely on the concealed Hosts panel's
descriptions or aria-hidden visual tooltip. Suggested bucket: other, accessibility/context loss for an available action,
no established security/process/data-loss route. Confidence: confirmed by source; no runtime/browser experiment needed
or performed. Temporal qualification: persists at pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b
attention_list.rs:299-305, which still has only data-tooltip and visible Update. proposed_drop:false. possible_cover:not
searched, parent owns queue/plan/filter matching. Caveats: screen-reader presentation may vary, but there is no explicit
label/description association in the markup; neighboring generic state remains available. New discovery, distinct from
known g008:g008-9@19 refresh-failure omission, local l2/g080:l2 dismissal ambiguity, and l1/c3/c4/c7's narrow
refutations.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g079/l9`; supporting records
`g080/i9`, `g081/f2`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
