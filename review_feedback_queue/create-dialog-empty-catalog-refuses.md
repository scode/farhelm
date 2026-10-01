# A failed model catalog read makes the create dialog reject saved settings

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If the create dialog's model list fails to load (or has not finished loading), cloning a session, using "replace with",
or applying a recent setup that names a reasoning effort shows "this saved choice is no longer supported" and disables
Launch. Only closing and reopening the dialog recovers.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F9 / COR-CATALOG-EMPTY`, tagged **definite**. Anchor and title:
`crates/farhelm-ui/src/list/create_form.rs:2242` — a failed or still-loading model catalog makes the create dialog
refuse saved launches as "no longer supported".

The create dialog reads the helm's launch catalog (which models and reasoning efforts each agent offers) once when it
opens. If that read fails, or has not finished yet, the dialog substitutes an empty catalog
(`.and_then(|r| r.as_ref().ok()).cloned().unwrap_or_default()` at
`crates/farhelm-ui/src/list/create_form.rs:2242-2247`). The compatibility check that guards saved choices
(`selection_is_compatible` in `crates/farhelm-ui/src/launch_composer.rs:1154`) rejects any selection that names an
effort when the catalog lists no efforts for it, which an empty catalog never does. So when the user clones a session,
uses "replace with", or applies a recent setup that specified an effort, and the catalog read failed or is still
pending, the dialog shows "this saved choice is no longer supported by the current catalog; choose a compatible model or
effort" and disables Launch (`create_form.rs:2306-2317`, `:3227`, `:3352`, `:3632`). The catalog is never re-read; only
closing and reopening the dialog recovers.

A hidden network error thus turns into a false statement that the user's saved settings are invalid. The "Restart with"
dialog already handles this correctly (`crates/farhelm-ui/src/restart_with.rs:206-207`): it treats the selection as
compatible while the catalog read has failed, since an outage cannot prove a choice invalid and the helm validates the
final request anyway, and it shows the read error. The fix is to track "pending" and "failed" separately from the model
list, skip the compatibility check at the three call sites unless the catalog is actually known, and show the read error
with a way to retry, as "Restart with" does.
