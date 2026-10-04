# Cancelled uninstall planning leaves SSH helpers

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Cancelled uninstall planning leaves SSH helpers.

## Details

`F12 / COR-UNINSTALL-CANCEL` — **definite** — `crates/farhelm-helm/src/provisioning/service.rs:1162` — Cancelled
uninstall planning leaves SSH helpers

Preparing an uninstall confirmation runs directly on the HTTP request’s task. That preparation calls the host-probe
backend, which starts an isolated SSH process group and a separate task to drain stderr. Explicit group cleanup and
waiting for the stderr reader occur when the probe completes; dropping the probe future runs only the direct child’s
`kill_on_drop` behavior.

Closing or reloading the page during planning can therefore kill SSH while leaving a configured helper, such as a
`ProxyCommand`, alive. If that helper keeps the inherited stderr pipe open, the detached reader can remain alive too.
Repeating this can accumulate helper processes and readers on the helm’s machine.

Run the planner on a helm-owned task so that its bounded probe reaches normal cleanup, or give the backend
cancellation-safe ownership of group termination, child reaping, and reader cleanup.

Suggested bucket: **other**. No possible cover was identified. The consequence requires a concrete helper that survives
SSH and holds inherited stderr; plain SSH and a local probe do not establish it. This concerns helm-side helpers, not
loss of remote user sessions. The discovery-probe fix at `TRIAGE_OUTCOMES.md:5910–5927` addressed another caller; the
analogous update-planning path already existed.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **other**.

Originating reviewers and passes: `helm_systems p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Concrete helper that holds inherited stderr required. Prior discovery ledger5910–5927 different
caller. UPDATE analogous preexists.

## Filed reviewer metadata

- `helm_systems p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable
