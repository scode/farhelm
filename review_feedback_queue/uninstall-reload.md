# Uninstall can lose process-only kill policy before stopping

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Uninstall can lose process-only kill policy before stopping.

## Details

`F10 / COR-UNINSTALL-RELOAD` — **possible** — `crates/farhelm-helm/src/provisioning/backend.rs:2473` — Uninstall can
lose process-only kill policy before stopping

Remote uninstall removes the supervisor’s service file before stopping the supervisor. It deliberately delays its own
systemd reload because the loaded service must retain `KillMode=process`: stopping should terminate only the supervisor,
leaving its private tmux server and retained sessions alive.

The stop command checks that policy and then invokes `systemctl stop` as a separate operation. Running both commands in
one shell does not prevent another ordinary systemd reload between them. With the service file already removed, that
reload can discard the process-only policy and restore the default control-group policy. The subsequent stop could then
kill the tmux server and its retained sessions, or sessions created after the uninstall session check.

Preserve an effective process-only policy across reloads until stopping finishes. Validate the chosen approach against
an isolated user manager, with a barrier that places an external reload between the policy observation and stop.

Suggested bucket: **highest**. No possible cover was identified. This interleaving has not been reproduced. The merged
report records that audit D19 excludes deliberate filesystem replacement but retains the ordinary reload case;
deliberate interference is not the premise here.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `helm_lifecycle p1`, `helm_systems p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Interleaving not reproduced. D19 audit excludes deliberate filesystem swaps only, says ordinary
reload remainder record.

## Filed reviewer metadata

- `helm_systems p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable
- `helm_lifecycle p1`: confidence as filed: **possible, likely**. Open premise: an unrelated user-manager reload occurs
  after the property read but before systemd processes the stop request. Suggested bucket as filed: **possible highest**
  — loss of session processes and retained terminal panes.
