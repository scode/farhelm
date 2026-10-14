# Host plans cannot receive keyboard scrolling focus

Reviewed commit: fb80a800629118f1ea43605f45d464d4f0114248

## TLDR

Keyboard-only desktop users may be unable to scroll through a setup plan before approving it.

## Details

Suggested bucket: `high`. Confidence: likely; WebKit's keyboard scrolling behavior for this exact DOM was not
reproduced. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

crates/farhelm-ui/src/provisioning.rs:812-815 creates a separate scrolling body containing only a preformatted plan.
assets/app.css:3049-3053 makes this body the scroll owner, but it has neither tabindex nor a focusable descendant.
Initial focus goes to Cancel through hosts.rs:3870-3873 and hosts/settings_dialog.rs:195-197; remaining controls are
outside the body. In engines that do not automatically tab-focus scroll containers, notably WebKit, ordinary navigation
cannot focus the plan region, and scrolling keys from the footer do not scroll a sibling. Previously the scroll owner
was the dialog containing the focused buttons. Trigger: an ordinary plan taller than the available body. Give the plan
region an explicit, labelled keyboard focus target and verify scrolling with the shipped engine family.

Possible cover: None found. SPEC_impl.md's full concrete action-list requirement at 3526-3527 supports inspecting the
complete plan.

Current-main status: Persists structurally at pinned main: provisioning.rs:817-820 still has no focus target and
assets/app.css:3204-3208 retains the separate scroll owner. Later confirmation-focus changes do not make the body
focusable.

Review coverage: full commit diff and relevant code paths read. Runtime behavior and harmful reliance were not
reproduced; the note preserves the reviewer's stated unverified premises.

## Additional affected scope

Reviewed commit: 1e0c3745462087e101bfe5f7f48d31ed1c1cff07

Confidence: likely — DOM/CSS mechanism and WebKit limitation established; the exact shipped desktop build was not
reproduced.. Suggested bucket: `high`. Source: dedicated `gpt-6.1-sol` high adversarial review. This extends the same
finding rather than adding a duplicate.

At crates/farhelm-ui/src/provisioning.rs:877–879, the plan moves into a separate text-only body without tabindex or
focusable descendants. The new host-dialog-framed class at :2826 makes the outer dialog overflow:hidden;
app.css:3029–3037 makes only that body scroll. Focus remains in sibling header/footer buttons, so keyboard scrolling
cannot reach the clipped plan in WebKit. Previously those buttons were inside the scrolling outer dialog. This affects
short windows, enlarged text, or long paths. Give the scrolling body a keyboard focus target and accessible name, and
verify keyboard scrolling with an overflowing plan. WebKit documents this platform behavior in
[bug 277290](https://bugs.webkit.org/show_bug.cgi?id=277290). The added tests at
e2e/tests/terminal-multihost.spec.ts:3855–3861 inspect text and geometry at height 850, without exercising keyboard
scrolling.

Possible cover: No matching Planned item, BUGS entry, queue item, filter, or searched triage decision found.

Current-main status: Persists structurally at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703:
provisioning.rs:885–887 still has no focus target; app.css:3077–3082 and 3200–3208 retain the scrolling arrangement. No
fixing commit identified.
