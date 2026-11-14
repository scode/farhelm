# Container-leftover Delete opens its question without focusing an answer

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: opening a container-leftover Delete question leaves keyboard focus on the old list
button instead of moving into the question. Keyboard users must find the answers themselves; another Enter on the
still-focused Delete just opens a replacement question. Source: containers_settings.rs:112-126 keeps the row button
mounted and enabled and its handler only clears action_error and calls confirm.open(row.clone(),()). The inline
alertdialog at :130-178 supplies neither autofocus nor an onmounted/use_effect focus handoff for either Cancel or Delete
container. These are all the component's hooks and answer controls. Settings itself is already mounted;
settings.rs:329-339 installs its root with remembered Sessions/close focus. modal_dialog.rs:957-967 explicitly keeps
child focus handoffs independent and ordinary rerenders from repeating initial focus; :999-1006 invokes handle.install
only on root mount. modal_isolation.rs:176-191 permits existing focus anywhere inside that Settings root and does not
move focus from the retained row; its observer at :202-213 only claims outside siblings, not new alertdialog answers.
The scroll-region recovery at modal_dialog.rs:589-614 likewise refuses recovery while a usable current control owns
focus. Thus the generic boundary does not supply the missing question handoff. Exact applicable contract, SPEC.md
Keyboard focus, Opening and closing (:3246-3252): "A confirmation the user just opened starts on the action they asked
for: focus starts on its enabled confirming button"; it then separately says "A risky action that cannot be undone
starts on Canc

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g077/f7`.

### g077/f7

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 16 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. Discovery pass 1. TLDR: opening a container-leftover Delete
question leaves keyboard focus on the old list button instead of moving into the question. Keyboard users must find the
answers themselves; another Enter on the still-focused Delete just opens a replacement question. Source:
containers_settings.rs:112-126 keeps the row button mounted and enabled and its handler only clears action_error and
calls confirm.open(row.clone(),()). The inline alertdialog at :130-178 supplies neither autofocus nor an
onmounted/use_effect focus handoff for either Cancel or Delete container. These are all the component's hooks and answer
controls. Settings itself is already mounted; settings.rs:329-339 installs its root with remembered Sessions/close
focus. modal_dialog.rs:957-967 explicitly keeps child focus handoffs independent and ordinary rerenders from repeating
initial focus; :999-1006 invokes handle.install only on root mount. modal_isolation.rs:176-191 permits existing focus
anywhere inside that Settings root and does not move focus from the retained row; its observer at :202-213 only claims
outside siblings, not new alertdialog answers. The scroll-region recovery at modal_dialog.rs:589-614 likewise refuses
recovery while a usable current control owns focus. Thus the generic boundary does not supply the missing question
handoff. Exact applicable contract, SPEC.md Keyboard focus, Opening and closing (:3246-3252): "A confirmation the user
just opened starts on the action they asked for: focus starts on its enabled confirming button"; it then separately says
"A risky action that cannot be undone starts on Cancel instead, as the YOLO confirmation does." This irreversible
whole-container cleanup should at least focus the appropriate answer (Cancel under the irreversible rule), not keep
focus on its old opener. Correction to the initial lead: the defect is absence of any question-answer focus handoff; it
is not a demand to focus the risky Delete answer contrary to that exception. Other sandbox orphan confirmations also
predate this focus policy, but this assigned new component creates its own new noncompliant route during the review
range; pre-existing similar behavior is no refutation. Suggested bucket: other, keyboard navigation/focus contract
defect. Could-be-highest:no established destructive mis-targeting, automatic confirmation, security or work-loss route;
retained focus is on the opener, which reopens rather than deletes. Confidence: confirmed through source and contract
analysis, not execution. proposed_drop:false. possible_cover:none in supplied neighbour projection. Fix direction: add
an opening-bound focus handoff to the question's intended initial answer, preserving repeated/composing Enter guards and
captured ConfirmToken answers. No runtime tests, reproductions or live-install actions.

Returned proposal (claims-050, event 16): **retain-verified**, suggested bucket **other**.

The new container-leftover question provides no opening-bound answer focus. Keyboard activation can leave focus on its
still-enabled opener; another Enter opens another question instead of reaching an answer. The finding does not choose
the risky confirming answer over Cancel.

Original stronger consequence: The initial lead suggested focus should reach the confirming answer. The final finding
correctly narrows that to an appropriate answer, with Cancel under the irreversible rule. Neither version establishes
wrong-target destruction, automatic consent or user-work loss.

Unresolved premise or limitation: No runtime focus observation or engine-wide pointer behavior measurement. The verified
route is keyboard activation while the opener owns focus.

Proposed basis: Retain missing question-answer focus independently of the unresolved Confirm-versus-Cancel default.

Existing parent disposition (event 16): verified. Parent explicitly accepted named retain-verified proposal from
claims-050 after inspecting it. The new container-leftover question provides no opening-bound answer focus. Keyboard
activation can leave focus on its still-enabled opener; another Enter opens another question instead of reaching an
answer. The finding does not choose the risky confirming answer over Cancel. Retain missing question-answer focus
independently of the unresolved Confirm-versus-Cancel default. Checked source/results:
["350a4b33b6d4476f4a78e43cea124bc2b828f462:crates/farhelm-ui/src/containers_settings.rs:112-121 keeps the opener enabled
and only opens confirm. The full component and :128-181 have no autofocus, onmounted focus or effect handoff for the
alertdialog answers.","Same revision:settings.rs:329-344 installs the already-mounted Settings root.
modal_dialog.rs:949-967 keeps child handoffs independent; :999-1008 installs only on root mount.
modal_isolation.rs:185-209 permits focus already inside Settings and observes outside siblings, not new answer focus.
modal_dialog.rs:580-618 declines recovery while a usable control retains focus.","SPEC.md Keyboard focus, Opening and
closing requires a just-opened confirmation to start on an answer; its irreversible-action exception affects which
answer, not whether a handoff exists. A keyboard opener supplies the focus premise without assuming pointer focus
behavior on every engine. No execution or destructive auto-confirmation was established."] Current coverage/results:
["Current SPEC.md:3261-3290 and SPEC_impl.md GUI focus were checked. General irreversible-default ambiguity does not
accept leaving focus on the opener.","TODO.md Planned empty. Searched unlanded plans/queue, BUGS.md, INDEX.md and active
TRIAGE_OUTCOMES.md for container-leftover question focus. No exact coverage.","Read queue items
orphan-confirmation-opens-outside-visible-scroll.md, trash-question-reopen-skips-answer-focus.md and
irreversible-confirmation-focus-contract.md in full. They concern a different component's long-list reveal, a mount-only
replacement-token lifecycle, and choice of default answer respectively. None fixes this new component's absent initial
handoff; no exact alias or extension identified.","FILTER.md does not match the ordinary opening sequence;
scrolling-only filtering does not cover the missing keyboard handoff."] Exact cited cover: [] Original stronger
consequence and missing premise remain preserved: "The initial lead suggested focus should reach the confirming answer.
The final finding correctly narrows that to an appropriate answer, with Cancel under the irreversible rule. Neither
version establishes wrong-target destruction, automatic consent or user-work loss."; "No runtime focus observation or
engine-wide pointer behavior measurement. The verified route is keyboard activation while the opener owns focus.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g077 record](records/review-through-00de0b6c/groups/g077.json). Record ID: `f7`.
