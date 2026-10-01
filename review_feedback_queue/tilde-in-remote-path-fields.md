# A `~` in the add-host path fields is taken literally

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

Typing `~/.local/bin/farhelm` in the add-host form's "remote farhelm" field makes Farhelm think it is not installed
there and offer setup, and `~/x` as the remote state dir creates a folder literally named `~` in the remote home.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F29 / COR-TILDE-PATHS`, tagged **possible**. Anchor and title:
`crates/farhelm-helm/src/provisioning/backend.rs:1148` — a `~` in the add-host "remote farhelm" or "remote state dir"
fields is taken literally.

The hosts panel's add-host form has two optional free-text fields, "remote farhelm" (where the farhelm binary lives on
the remote machine) and "remote state dir" (`crates/farhelm-ui/src/hosts.rs:3124-3150`). The UI gives no hint about
their format. The helm validates them only loosely: the binary path just needs a file name (`remote_farhelm_is_usable`,
`store.rs:1577`). When the helm probes the host over ssh, it builds a small remote shell script (`probe_script`,
`backend.rs:1148-1160`) with both values single-quoted (`shell_path`, `:2269`), and single quotes suppress the shell's
`~` expansion.

So entering `~/.local/bin/farhelm` makes the probe run `command -v '~/.local/bin/farhelm'`. That fails, the script exits
with its "no farhelm here" status (75), and the hosts panel offers to set Farhelm up on a host where it is already
installed. A state dir of `~/x` reaches the remote `farhelm internal stdio --state-dir '~/x'` as a relative path, which
resolves against the ssh login directory, i.e. `$HOME/~/x`, a folder literally named `~`. SPEC.md documents `~`
expansion only for session working directories (around SPEC.md:361), not for these fields. One mitigating detail: an
in-place _update_ of a host whose remote farhelm is not absolute is refused with a clear "needs an absolute
remote_farhelm" message (`plan.rs:503`). The probe has no such check.

This is tagged possible because it assumes users will type `~` here. Nothing in the UI or docs tells them not to.
Suggested change: either reject values starting with `~` at the API boundary with "use an absolute path", or expand a
leading `~/` to `"$HOME"/` inside the remote script.
