# Quick Switcher can act on a template the user has not seen

Reviewed commit: 959cde41e6d525db005de9da5b16862283cb7da5

## TLDR

If the template list changes while Quick Switcher is open, Enter can start a different saved setup from the one the user
still sees selected.

## Details

The Enter handler resolves the current exact match when it runs. A template update can change that match before the new
frame has reached the screen. Bind the key to the presented choice or wait for the new choice to be shown. Keep the
existing rule that a confirmed template name uses its current definition.

At reviewed tip `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see
`crates/gui/farhelm-gui/src/session_list/quick_switcher.rs:585–635`,
`crates/gui/farhelm-gui/src/session_list/quick_switcher.rs:720–729`, and `SPEC_impl.md:8586–8587`. The in-range search
aliases reach the same route. Source-confirmed by g019/l5. Launch still follows normal admission rules.

Suggested bucket **highest** for starting a saved command before its selection has been presented. No command was
launched during the review.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g019/l5.
