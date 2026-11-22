# The launcher’s documented Tab order omits its conditional command-notice stop

Reviewed commit: f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6

## TLDR

The specified Tab order omits a command-message stop that can appear before the form.

## Details

**other.** Source inspection.

f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6 SPEC.md:1435-1439; crates/gui/farhelm-gui/src/launcher/view/search.rs:570-588;
crates/gui/farhelm-gui/src/components/dialog.rs:442-464;
crates/gui/farhelm-gui/src/components/scroll_box.rs:10-15,37-42,170-186; 1a53d9d35889d7dde96e31320cc1681dd013737d
SPEC.md:1446-1449,3740-3747; crates/gui/farhelm-gui/src/launcher/view/search.rs:673-719;
crates/gui/farhelm-gui/src/components/dialog.rs:451-475,681-688

When there is a command-field message, the launcher adds a named tab stop in the header before the form, after Close
when button traversal includes Close. The launcher-specific guide instead goes straight from the command field and Close
to the visible body controls. The first implementation and the pin both conditionally place a ScrollBox under the header
row for a template refusal, held-field refusal or repository note. The ScrollBox counts as a text field for traversal
and registers its own stop even with a one-line message. The general accessibility rule at pinned SPEC.md:3740-3747
already requires a named stop for scrollable regions that hold no focusable content. Document the conditional step in
the launcher-specific traversal description. This concerns the header messages box, separate from any scroll access
issue for result rows or a results pane. Verified from registration and layout, without runtime focus or
assistive-technology checks.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-contract/LC-14`.
