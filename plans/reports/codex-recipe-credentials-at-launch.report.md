## What this was about

When Farhelm delivered a Codex recipe endpoint to a sandbox or container, it rewrote Codex's settings file. Delivery
could overwrite an edit saved at the same time. The maintainer chose to supply the endpoint when Farhelm starts or
resumes Codex and to clear the old provider selection and bearer once on hosts where Farhelm recorded an endpoint write.
The provider still uses the same ID so Restart and opening an existing conversation by ID keep working.

## Things you should know

Restart a running Codex session to pick up a replacement endpoint token. Typing `codex` in a terminal tab uses that
shell's usual Codex configuration. A token preconfigured on the `farhelm` provider in a base image can still go to the
recipe's URL even if the recipe's token picker says **None**. Choose **None** as the Codex mode to use the image's
configuration. Codex Subscription still uses `auth.json`.

If Farhelm can't clean up an older settings write, it warns on the host and withholds the new Codex endpoint until a
retry succeeds. Claude's recipe credentials can still reach that host. A host recorded as having no managed Codex
endpoint may have settings the user wrote later. The migration leaves those settings alone.

## Open questions and possible follow-ups

None.

## PRs

- [#4679](https://github.com/scode/farhelm/pull/4679/changes) — Supply Codex recipe endpoints at launch and refresh
  Settings help and screenshots.
- [#4680](https://github.com/scode/farhelm/pull/4680/changes) — Update the specs and the Tensorlake sandbox guide, and
  remove both TODO entries.

## Checks

- 63 targeted Rust tests passed on `54d04289`, covering Codex provider arguments and the private credentials file,
  scoped delivery failures, the protocol bump and recipe editing. Three terminal-host tests passed, including named
  Codex Start and Restart with a rotated token and rejection of a supervisor one protocol version behind. Recorded runs
  `8ae1665c-12f8-4323-8bc4-7389a7f17e40` and `846a6d1d-3d29-430b-a878-38f6b4f030d4`.
- A real local Podman lifecycle test passed with both agents' credentials in the same private file and no Codex settings
  file created. It ran the same-source Linux payload (run `29a30a1e-4abc-45e4-a69d-a2ae149639a9`). Strict all-targets
  Clippy, Rustfmt and the test-delay check passed. The website built and all internal links validated (run
  `536e5718-6696-4b7d-9215-e1fca6910471`).
- A full Linux reference capture changed four expected pictures. Both themes were reviewed, the references were
  published, and the recipe editor scenes matched them (runs `f09e14fa-e80f-4014-9861-937a7164bc0f` and
  `9db05451-b571-431c-95cb-4f4140532279`). Markdown formatting and changelog format passed.
- Reused the targeted Rust and Podman results after rebasing. Main added sandbox ID validation to Tensorlake
  Suspend/Resume and added feedback for Stop and suspend. The credential delivery and launch paths, the Podman test
  actions, and the recipe editor scenes used by the checks were unchanged.
- No live Tensorlake run was needed to exercise delivery. Both providers use the same delivery code, and the real local
  Podman test exercised it with a matching supervisor. Stand-in agents let the launch tests observe exact arguments and
  environment variables without sending vendor requests. The selected suites covered the changed paths, so the full
  workspace suite was skipped. CI runs on demand and was skipped for the draft stack.

## Review gate

Both independent reviews cleared after fixes, with no outstanding findings. The source review found that a historical
no-endpoint record could authorize a settings edit and that the Restart test could mistake replay for fresh readiness.
The fixes preserve user settings after that record and require a distinct readiness signal from the replacement process.
The independent documentation review checked the one-time cleanup, connection timing, tab behavior and image-baked token
caveat. Both PR messages passed separate cold reads.
