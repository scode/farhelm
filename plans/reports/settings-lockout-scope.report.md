### What this was about

Updating one host made Settings unavailable on every host until the update finished. The lockout was meant to prevent
setup or uninstall confirmations from opening alongside Settings and making both dialogs unresponsive. An ordinary
Update opens no confirmation. The maintainer asked to keep that protection where needed while allowing Settings on
unrelated hosts during an Update.

### Things you should know

Another host's Settings now opens during Update planning and while the accepted update runs. The host doing the work
keeps its own Settings unavailable, including before submission. Settings still waits on every host while a setup or
uninstall confirmation is being prepared or held, including the local host's automatic setup check. The brief shared
operation lock during submission also still blocks Settings everywhere.

The broader busy state still controls setup, Update, rerun, planning labels and update availability exactly as before.
The implementation publishes the narrower confirmation state separately; it conservatively includes any pending plan,
including a defensive Update-plan branch that the ordinary flow does not use. Modal isolation itself is unchanged. The
implementation specification now states the distinction, and the covered TODO and review-queue finding were removed.

### Open questions and possible follow-ups

None identified within this scope. The browser fixture uses an injected host-side executor; it proves the real helm/UI
ordering and dialog behavior, not installation on a real remote host. WebKit covers the desktop app's engine family; no
native Mac runtime check was needed for this shared UI change.

### The PRs

- [#2032: keep other hosts' Settings usable during updates](https://github.com/scode/farhelm/pull/2032/changes): one
  draft fix, including the regression tests, implementation-spec wording, fixed changelog fragment and covered
  queue/TODO removal.

### Checks run, reused and skipped

- Run: recorded
  `cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(provisioning::tests::) | test(hosts::tests::)'`, 54
  passed and 579 filtered out; run `7c790bdd-9692-41cc-b75b-7647554b1b64`. Pinned nextest and tmux, four slots, zero
  retries, no runtime substrate skips. The new pure-state test distinguishes confirmation preparation and pending plans
  from Update ownership, while pinning unchanged provisioning offers.
- Run: recorded Playwright selection in `tests/provisioning.spec.ts`, 18 passed across Chromium and WebKit; run
  `6bba6aee-f03e-4878-8b0f-71ae6bb190f8`. One worker, zero retries, pinned tmux; fixture cleanup completed. The new test
  holds Update planning, verifies no update run has begun, refuses the updating host's Settings and opens another host's
  Settings. The existing held-running Update test now opens the bystander's Settings. Neighbors cover the unchanged
  uninstall lockout, accepted setup, local automatic confirmation and phase transition, Update all and the page-lock
  wait.
- Run: `cargo clippy -p farhelm-ui --all-targets --features desktop -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo build`, and the release web build. The affected desktop
  renderer compiled through Clippy. `cargo fmt --all -- --check`, targeted Markdown `dprint check`, the diff whitespace
  check, changelog format lint and isolated `python -B scripts/check-test-sleeps.py` passed; 299 delays inspected, zero
  missing reasons. A separate documentation pass covered every touched file and the new lifecycle distinction.
- Reused: those checks across the careful rebase from `17092ee8` to `33bdd64a`. Every incoming diff was read: queue
  claims and another plan's report delivery only. The rebase changed no tested behavior. The only post-review source
  edit clarified the existing method's docstring; no new runtime check was warranted for that prose.
- Skipped: full workspace and browser batteries, hosted CI, native Mac runtime, website, installer and release gates.
  Targeted unit and both-engine browser checks cover the changed lifecycle and its neighboring flows; no API, schema,
  desktop bridge, website or release machinery changed. No earlier runtime passes were reused from another plan.

No runtime check failed. Initial worker setup found a missing Cargo PATH and a container-home permission problem before
the web build could compile; those attempts remain recorded separately from successful execution.

### Review gate outcome

The required fresh-context gpt-6.1-sol reviewer at high effort read the full diff and supplied test-authoring contract.
No findings. The review checked the modal-producing paths, unchanged broad-state semantics, both Settings guards and the
regression tests' planning/running premises. This was static inspection; runtime evidence is separate above. The final
commit and PR wording passed a fresh gpt-6.1-sol reader at medium effort.

Implementation stayed in the main agent under the plan's no-workhorse requirement. Required review artifacts and
decisions are retained privately. External review usage is retained without inferring exact reported model attribution;
native and main-agent usage are unavailable. The separate native report cold read is part of delivery.
