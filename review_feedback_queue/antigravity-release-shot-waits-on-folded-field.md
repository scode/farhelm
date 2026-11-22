# The supported Antigravity release-note recapture waits for a folded conversation field

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The Antigravity release-note recapture waits for a conversation field that stays folded under More options.

## Details

**other.** Source inspection.

scripts/docs-shots/release-notes.py:92–97,149–157; crates/gui/farhelm-gui/src/launcher/view/more_options.rs:27–60 at
1a53d9d35889d7dde96e31320cc1681dd013737d; introduced by da5615e0f51fcedc28c6f780206197d7de4f741d

An explicit correction of the Antigravity release-note shot reaches its current declaration and times out during
staging. The supported --local --recapture-release-shot route is documented in docs/docs-shots/SPEC.md:96–105.
scripts/docs-screenshots.sh:50–91,135–146 checks and runs that selection, including preview with --only release-notes.
scripts/docs_shots.py:65–86,110–163,464–476 loads and selects the declared shot, which is frozen in
website/src/data/docs-shots.json:368 and website/src/release-notes/v0.42.0.json:15. The capture engine resets, then
calls shot.steps before parking (scripts/docs_shots.py:421–433). Those steps open New, click Antigravity, and wait for
launcher-conversation with a bounded visibility poll (docs_shots.py:297–302;
vendor/gpui-agent/crates/cli/src/main.rs:1270–1298). The fresh launcher starts folded (launcher/view.rs:893). The
requirement only applies to Restart with, and a blank selector and wrapper trigger neither error
(launcher/request.rs:516–555). Choosing the harness can raise a separate inline hook question
(launcher/view/choices.rs:89–101; launcher/view/hook_question.rs:63–87,107–113; launcher/view.rs:2594–2622), which does
not expand the disclosure. Open More options before resolving the conversation region, and then address the later stale
park target in LV-8. The reviewer suggested other. Confidence: confirmed from source.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-visual/LV-9`.
