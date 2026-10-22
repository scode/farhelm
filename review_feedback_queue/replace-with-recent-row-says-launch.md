# In Replace with, Enter on a recent setup performs the replace while the row says only "launch"

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

When you open Replace with on a session, the launcher still lists recent setups, each marked "⏎ launch". Pressing Enter
on one creates the new session and then deletes the original one, its stored state and anything still running in it. The
row, its tooltip and what a screen reader reads for it never mention replacing or deleting. Only the replace button
carries that wording and the warning about what is still running, so a screen-reader user can delete the session without
having been read the warning on the control they pressed.

## Details

Found by the 2026-10-22 prioritized persistent review, group `172-ui-create-form-8of9` (finding f1, as revised by the
reviewer).

- `crates/farhelm-ui/src/list/create_form.rs:5565-5585`: Enter on a recent row applies the entry and calls
  `resubmit_composer()` (660-664, `requestSubmit` on the launcher form). In Replace with that is the replace, with the
  deletion precondition taken from the shown source state (around 2899-2903 and 1480-1490).
- The row's hint is the constant "⏎ launch" (5640). Its `data-tooltip` and `aria_label` (around 5550-5557) name only the
  folder, host and choices. The replace warning is the `aria_describedby` of the replace button only (around 4985); the
  warning itself (around 5080-5088) is a polite live region, so a screen reader may announce it when it appears or
  changes, but it is not the row's description. Recent rows render the same in every launcher mode.
- The Enter-launch itself is specified: SPEC.md "Creation", recent setups, "pressing Enter on a focused row launches the
  filled setup through the ordinary Launch path" (around 823-824), and SPEC.md "Replace with" says Clone, Replace with
  and New are "one launcher — same layout, same controls" (around 1241-1242). The same section also says "Its launch
  button is the confirmation of that delete: while the source has anything alive, the launcher shows Replace's warning
  about the source beside it" (around 1243-1244), and SPEC.md (around 1284) says "a destructive confirmation authorizes
  only what the prompt the user answered said would happen". What remains is that the row is a second confirming control
  that neither says "replace" nor carries the warning for assistive technology.

How to verify: open Replace with on a running session whose host has launch history, focus a recent row with a screen
reader, note its name and hint, press Enter, and see the source deleted.

Fix: in Replace with, show the hint as "⏎ replace" and add the replace warning to the rows' `aria-describedby`. The
alternative, making row Enter only fill the form and leaving submission to the replace button, needs a spec change.

Suggested bucket: high. Raised for a look because it concerns a destructive action; not claimed highest because the user
deliberately opened Replace with and the visible warning sits above the recent setups.

Confidence: confirmed by code reading that row Enter submits the replace and that the row's text and description do not
mention it. Likely, with the unverified premise that this misleads a user who opened Replace with on purpose.

Reviewer's proposed drop, kept for triage: read together, SPEC.md's recent-row Enter rule and "same controls" make a
recent-row Enter in Replace with the specified replace, and what remains may be an accessibility or wording issue
outside the review's charter. That covers the behavior, not the row's wording or description, so recording did not drop
it.

Possible cover, for triage to judge: SPEC.md "Creation", recent setups (around 822-826); SPEC.md "Replace with" (around
1235-1248) and the destructive-confirmation rule (around 1284-1299). No queue item, TRIAGE_OUTCOMES.md heading or
FILTER.md filter was found.
