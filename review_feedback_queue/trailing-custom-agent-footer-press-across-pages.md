# The custom agent editor's footer Save and Discard keep one element id on every custom agent page, so a held press can finish on another agent's draft

Reviewed commit: b2d540a8dacfb94140e5eb0927d78e447ce46111

## TLDR

In Settings, with unsaved edits on two custom agents (or on a custom agent and Add custom agent), a mouse press started
on one agent's Save or Discard and released after the keyboard switched to the other agent's page lands on that page's
Save or Discard. Release on Save sends the other agent's draft, which the person never chose to save; release on Discard
throws away the other agent's unsaved edits.

## Details

dcb70eb551 made the page footer's buttons page-named (`settings-footer-save-{key}`, editing.rs:612/630) precisely
because GPUI completes a click when mouse-down and mouse-up hit the same element id, so a press held across a page
change finished on the next page's identically placed Save (its new test
`a_press_held_across_a_page_change_saves_nothing`, tests.rs, pins that). Custom agent pages do not use that footer:
settings.rs:1865-1870 draws `custom_agents::Editor::footer` instead, and that footer's buttons use the fixed ids
`custom-agent-discard` (custom_agents.rs:750) and `custom-agent-save` (custom_agents.rs:760) for every registered agent
page and for Add custom agent. Each registration keeps its own editor and draft across page switches
(`SettingsDialog::custom_agents`; `custom_editor`, custom_agents.rs:181, picks the shown section's editor), and the
handlers act on the editor whose footer was drawn at release time (`editor.save` / `editor.restore` on the drawn
editor's weak handle), with no `ActionFrame` presentation check. So: dirty agent A and B; mouse-down on A's Save;
keyboard to B's nav button and Space (the same steps the new test uses); B's footer draws `custom-agent-save` at the
same pinned footer spot; mouse-up clicks it and saves B. Discard the same way calls B's `restore`, losing B's unsaved
typing. SPEC.md, "One GUI at a time" (a pointer action uses the target from the frame the user was shown) and
SPEC_impl.md, "GUI: Dioxus" ("Answers bind to what was drawn") cover this; the new SPEC_impl paragraph only claims the
page's own footer. Verify by reading custom_agents.rs:713-783 and settings.rs:1860-1930; a test mirroring
`a_press_held_across_a_page_change_saves_nothing` across two custom agent pages would show it. Fix: name the editor
footer buttons by the registration (e.g. `custom-agent-save-{section key}`) and, as the page footer now does, bind the
press to a presented snapshot of the draft version it offered.

Reviewer revision notes: Revision (synthesis turn): kept; the confidence premise re-checked and strengthened, no other
change.

### Triage notes

- Recorded bucket: `high`. The reviewer suggested other but offered high if acting on an unchosen draft is weighed like
  the page-footer case the commit fixed; recorded as high because the bucket is uncertain. Confidence likely: the shared
  element id premise is supported by source, not observed.
- Suggested bucket (reviewer): other (narrow gesture; unintended save of a draft the person typed, or loss of unsaved
  edits, not loss of stored data). Consider high if the maintainer treats acting on an unchosen draft as a trust-the-UI
  violation of the same weight as the fixed page-footer case.
- Confidence: likely. Unverified premise: that the two editors' footer buttons resolve to the same GPUI global element
  id path. Source supports it: the footer actions go through `DialogFrame::footer_actions` (components/dialog.rs:288,
  577), whose ancestors carry only ids built from the dialog id ("settings"), never the section; this is the same
  premise the commit's page-footer fix and its held-press test rely on. Not observed by execution.
- possible_cover: none known
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g06/f1 (finding), group g06 "Custom agent draft naming and the Settings footer binding", findings log line 4.
