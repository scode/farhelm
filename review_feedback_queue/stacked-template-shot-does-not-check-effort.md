# The stacked-template screenshot checks that High is visible, not that it is selected

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The stacked-template capture can pass without verifying that the selected effort matches its caption.

## Details

**other.** Source inspection.

scripts/docs-shots/launch-templates.py:49–64,149–160; scripts/docs_shots.py:297–302;
vendor/gpui-agent/crates/cli/src/main.rs:761–797,1270–1276 at 1a53d9d35889d7dde96e31320cc1681dd013737d; introduced by
f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6

The screenshot can show Default while its callout says the template set effort. Bridge.wait forwards the id. The CLI
matches the id path or path suffix and succeeds when the node has nonempty visible bounds. It reads no selected state.
The form builds one segment per offered effort and supplies the actual selection separately
(launcher/view/choices.rs:265–274,305–333). The radio group derives each id from the option label even when unchosen,
and emits a separate true or false toggled state (components/radio_group.rs:572–587,636–648,676–683). The second
template is followed only by the folder-value wait. The draft fixture templates/apply_tests.rs:194–232 applies a Codex
High template and a host/folder template in one batch and asserts High survives, covering the composition rule in
SPEC.md:1214–1217. launcher/search_view_tests.rs:923–947 covers two consecutive tl: choices by Enter, asserting folder
then launch kind and command. launcher/view_tests.rs:1497–1519 asserts the High toggle in Clone. The closest goldens
open each template in the editor separately (goldens/templates.rs:121–137), while the launcher results goldens stop at
rows or assert provenance for a recent setup (goldens/launcher.rs:386–444). None of these checks the displayed High
toggle after the two pointer picks in this docs capture. Assert the selected state after both picks to verify that
output. Confidence: confirmed for the capture check only. The reviewer suggested other. Possible coverage: the cited
draft, keyboard and Clone fixtures cover constituent behaviors, not the stacked capture assertion.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-visual/LV-2`.
