# Explicit Tailscale SSH host support

Written against main at 10973c17e on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term"). Verbatim:

> **Use the Tailscale SSH wrapper for Tailscale hosts.** If the user is using Tailscale, use the `tailscale ssh` wrapper
> instead of relying on the user manually getting known hosts initialized, so Farhelm can immediately use any Tailscale
> host it has ssh access to without fighting ssh known hosts. Separate from, and orthogonal to, the in-band status entry
> above. Not investigated yet, including how the wrapper fits Farhelm's non-interactive, connection-sharing ssh use.

In product terms: users can mark an SSH host as a **Tailscale host** when adding it (or toggle it later in Host settings
→ Connection, or set `tailscale: true` in `--ensure-hosts`). Farhelm resolves the destination through the local
Tailscale daemon, verifies the host's SSH key against the keys advertised by Tailscale using a per-host private
Farhelm-owned `known_hosts` file (never reading or touching `~/.ssh/known_hosts`), and routes through `tailscale nc`
when Tailscale is running in userspace-networking mode—while keeping all of Farhelm's OpenSSH security overrides,
connection sharing, and interactive SSH sign-in intact.

### Where it stands (from planning research and planning review at main 10973c17e; line numbers approximate)

- **Why Farhelm does NOT exec `tailscale ssh` directly.** Verified against Tailscale v1.104.7:
  `tailscale ssh [user@]<host> [args...]` accepts no OpenSSH `-o` flags, `-O check`/`-G`/`-O exit`, or `--`; it appends
  all trailing arguments as the remote command. Running it directly would drop all six mandatory `CONNECTION_OVERRIDES`
  (`ForwardAgent=no`, `ForwardX11=no`, `ClearAllForwardings=yes`, `RemoteCommand=none`, `RequestTTY=no`,
  `PermitLocalCommand=no`), `BatchMode=yes`, `ControlMaster` connection sharing, interactive sign-in probing, and the
  `--` option-injection guard.
- **What `tailscale status --json` provides.** Verified locally: top-level `TUN` (boolean: `false` means userspace
  networking), `MagicDNSSuffix`, `Self`, and `Peer` entries carrying `HostName`, `DNSName`, `TailscaleIPs`, and
  `sshHostKeys`. That provides everything `tailscale ssh` uses internally, without breaking OpenSSH flags.
- **Async preparation vs. synchronous pure argv lowering (`crates/farhelm-helm/src/ssh.rs`, `access.rs`).**
  `SshAccess::resolve`, `RemoteRoute::command`, `BaseRoute::command`, `ssh_base_args`, `ssh_sign_in_args`, and
  `ssh_control_exit_args` are all **synchronous and pure**. Running `tailscale status --json` and writing the private
  `known_hosts` file must happen at an explicit **async preparation boundary** (for example an async
  `tailscale::prepare(...)` or `SshAccess::prepare(...)` called before spawning at `SystemTransport::connect`,
  `SystemTransport::sign_in`, `provisioning::backend`, `provisioning::setup_sign_in`, and `environment::podman_probe`),
  producing a resolved `TailscaleSshAccess` that synchronous argv lowering turns into OpenSSH flags.
- **Hardcoded `SshAccess::UserConfig` / `HostKind::ssh_access` call sites to update:**
  - `crates/farhelm-helm/src/store.rs` (about 1034): `HostKind::ssh_access(self)` takes only `HostKind`; because
    `HostKind` stays `Ssh` and `tailscale: bool` lives on `HostRow`, move/add `HostRow::ssh_access(&self)` and update
    callers (`access.rs:389`, `transport.rs:215,241`, `provisioning/service.rs:956`).
  - `crates/farhelm-helm/src/access.rs` (about 436-445): `base_route(row)` currently hardcodes
    `SshAccessKind::UserConfig` for `HostKind::Ssh` — update it to read `row.ssh_access()` so nested Podman on a
    Tailscale SSH host works.
  - `crates/farhelm-helm/src/access.rs` (about 238): `RemoteRoute::stdin_idle_limit_secs()` matches `SshAccessKind`
    exhaustively; map `SshAccessKind::Tailscale` to `None`.
  - `crates/farhelm-helm/src/ssh.rs` (about 304): `ssh_sign_in_args` currently hardcodes `&SshAccess::UserConfig`.
  - `crates/farhelm-helm/src/ssh_sign_in.rs` (about 132, 155, 187): `SignIn::setup_ready` and `SignIn::establish`
    hardcode `&SshAccess::UserConfig`.
  - `crates/farhelm-helm/src/provisioning/setup_sign_in.rs` (about 35, 74): `Pending` stores only `destination: String`
    and `SetupSignIn::destination` drops `SshAccessKind`; carry `SshAccessKind` so provisioning sign-in preflight uses
    Tailscale access too.
- **Private `known_hosts` quoting vs. `ProxyCommand` quoting.** `crates/farhelm-helm/src/tensorlake/ssh.rs` has no
  `known_hosts` writer (OpenSSH writes Tensorlake's file itself via `accept-new`); do NOT generalize
  `tensorlake/ssh.rs`. Reuse `crate::ssh::ssh_config_path_option("UserKnownHostsFile", ...)` (`ssh.rs`, about 429) for
  the `known_hosts` path. Because `ssh_config_path_option` forbids `%`, write a small dedicated formatter for
  `ProxyCommand` that shell-quotes the `tailscale` binary path, escapes `%` as `%%` in that path while keeping literal
  `%h %p`, and refuses `${`, newlines, and control characters.
- **Host persistence and API (`crates/farhelm-helm/src/store.rs`, `hosts.rs`, `ensure.rs`, `provisioning/http.rs`,
  `provisioning/plan.rs`, `crates/farhelm-proto/src/ui_channel.rs`).** Every boolean host setting on `hosts.rs` uses its
  own verb route (`POST /api/hosts/{id}/yolo-without-asking`, etc., `hosts.rs:396-590`); a dedicated
  `POST /api/hosts/{id}/tailscale` with `{ "tailscale": bool }` (`#[serde(deny_unknown_fields)]`) fits the existing API
  cleanly and avoids overloading `POST /api/hosts/{id}/destination`.
- **GUI surfaces (`crates/gui/farhelm-gui/src/hosts/add.rs` and `settings.rs`).**
  - Add host dialog (`add.rs`, `WIDTH = 560px`): SSH tab stacks `SSH destination` input + help text, optional inline SSH
    sign-in form, `Alias (optional)` input + help text, error line, and Cancel / Add footer.
  - Host settings dialog (`settings.rs`, `WIDTH = 480px`): Connection section holds `Destination` (with Edit) and
    `Alias` (with Edit), followed by Appearance, Checkouts, and Permissions (`Checkbox::switch()`). The GUI enforces
    in-flight launch refusal via `super::launch_in_flight` (`settings.rs`, about 21-23), while the helm serializes
    against provisioning via `host_provision_lock` + `host_write_lock` (`hosts.rs`, about 879, 907).

### Acceptance criteria

1. **Tailscale CLI discovery & peer resolution (new module in `farhelm-helm`).**
   - Discover the `tailscale` CLI on `PATH`, and on macOS also fall back to
     `/Applications/Tailscale.app/Contents/MacOS/Tailscale` when not on `PATH` (injectable for tests, never mutating
     process env).
   - Parse `[user@]<host>` cleanly from the host's destination (refusing leading-dash/malformed inputs safely).
   - Run `tailscale status --json` asynchronously (bounded timeout) to resolve `<host>` against peers (and `Self`) by
     `TailscaleIPs`, exact `DNSName` (with or without trailing dot), or short `HostName` / MagicDNS prefix
     (case-insensitive, refusing ambiguous short names with a clear error naming the full MagicDNS name to use). Accept
     both `sshHostKeys` and `SSH_HostKeys` via serde alias.
   - Extract the peer's advertised host keys and target address (preferred Tailscale IP); if host keys are empty, fail
     with a clear user-facing error explaining that Tailscale does not advertise SSH host keys for that node.
   - When `tailscale` is missing, stopped (`BackendState != "Running"`), or the peer is not found on the tailnet, return
     a clear, plain-English error without touching `~/.ssh/known_hosts`.
2. **Per-host Farhelm-owned private `known_hosts` and system `ssh` flags (`SshAccessKind::Tailscale`).**
   - Write the resolved peer's host keys into a **per-host/per-peer** private file under `<state_dir>/tailscale/` (for
     example keyed by a hash of the destination, written atomically via temp-file + rename with `0600` permissions — no
     shared-file locking and automatic stale-key replacement on key rotation). Never read or modify
     `~/.ssh/known_hosts`.
   - Keep the user's `[user@]<host>` after `--` and pass `-o HostName=<resolved-ip>` (keeping `ControlPath=%C` stable
     and preserving user `~/.ssh/config` `User`/`IdentityFile` matching if any, while preventing system DNS failures
     when `--accept-dns=false`; if a non-default port is configured, note port-22 scope or account for `[host]:port` —
     decide and log as a DECISION). Write host-key lines matching the hostnames/IPs OpenSSH checks (`<resolved-ip>`,
     `<dns-name>`, `<original-host>`) so `StrictHostKeyChecking=yes` matches cleanly.
   - Pass OpenSSH options alongside all 6 mandatory `CONNECTION_OVERRIDES`, `BatchMode`, `ControlMaster` / `ControlPath`
     / `ControlPersist`, keepalives, and `--`:
     - `-o UserKnownHostsFile=<farhelm-per-host-known-hosts>` (formatted via `ssh::ssh_config_path_option`)
     - `-o GlobalKnownHostsFile=/dev/null`
     - `-o StrictHostKeyChecking=yes`
     - `-o UpdateHostKeys=no`
     - `-o CanonicalizeHostname=no`
     - When `tailscale status --json` reports `TUN == false` (userspace networking), also pass
       `-o ProxyCommand=<quoted-tailscale-bin> nc %h %p` using a dedicated formatter that escapes `%` → `%%` in the
       binary path while keeping `%h %p` intact and rejecting `${`/control chars.
   - Keep `SshAccessKind::asks_user()` enabled (`true`) for Tailscale hosts so interactive SSH sign-in continues to work
     for ordinary `sshd` over Tailscale while remaining a no-op when Tailscale SSH authenticates automatically.
   - Refresh the per-host private `known_hosts` asynchronously at the preparation boundary before probing, provisioning,
     signing in, and connecting (or reconnecting after failure).
   - Update all six call sites named under "Where it stands" so supervisor transport, provisioning, nested Podman,
     interactive sign-in (`-O check`, `-G`, establish), and ControlMaster release (`-O exit`) all honor Tailscale
     access.
3. **Persistence, helm API, and `--ensure-hosts`.**
   - Add `tailscale INTEGER NOT NULL DEFAULT 0` on `hosts` in `helm.db`, following the helm's migration ladder and
     historical-schema fixture tests. Keep `HostKind::Ssh` and `hosts_ssh_destination`.
   - Expose `tailscale: bool` (default `false`) on `HostView` (`farhelm-proto/src/ui_channel.rs`), `HostSpec`
     (`POST /api/hosts`, `--ensure-hosts` JSON5), `POST /api/hosts/probe` SSH target, and `ProvisioningTarget` /
     `RemoteRoute::Ssh`. Keep `POST /api/hosts/{id}/destination` dedicated to destination edits (do not silently ignore
     a `tailscale` field there).
   - Add dedicated verb route `POST /api/hosts/{id}/tailscale` with `{ "tailscale": bool }` (`deny_unknown_fields`). On
     the helm, take `host_provision_lock` + `host_write_lock` (like `set_destination`), call
     `release_shared_connections(&old_row)` **before** mutating the DB row (degrading cleanly — log and continue — if
     Tailscale is currently stopped while toggling `true -> false`), commit the toggle, and reconnect the host actor.
     Clean up the per-host private `known_hosts` file when a host is deleted or toggled off (best-effort).
4. **Desktop GUI (matching the settled planning mockup) and golden screenshots.**
   - **Add host dialog (`crates/gui/farhelm-gui/src/hosts/add.rs`):** On the SSH tab, directly below `SSH destination`
     and its help text (which now mentions "user@host, a Tailscale node name, or a Host name from your ssh config"), add
     a checkbox **`Tailscale host`** with helper text: _"Verify the host's SSH key through Tailscale instead of
     `~/.ssh/known_hosts`, and resolve Tailscale node names."_ Wire it into probing and adding.
   - **Host settings dialog (`crates/gui/farhelm-gui/src/hosts/settings.rs`):** In the **Connection** section for SSH
     hosts (below `Destination` and `Alias`), add a **`Tailscale host`** switch (`Checkbox::switch()`) with helper text
     explaining that Farhelm verifies host keys and routes through local Tailscale instead of `~/.ssh/known_hosts`.
     Enforce the GUI's in-flight launch refusal (`super::launch_in_flight`) just like destination edits, save via
     `POST /api/hosts/{id}/tailscale`, and show error/refusal feedback if busy.
   - Update affected GUI golden screenshots (Add host SSH scene and Host settings SSH scene, light & dark) via
     `scripts/check-gui-goldens.py` and `scripts/gui-goldens.py publish` per `.agents/validation.md`.
5. **Tests, without sleeps (`.agents/test-authoring.md`):**
   - Unit tests for Tailscale peer resolution (IP, FQDN with/without dot, short hostname, case-insensitivity, ambiguous
     short name refused, empty `sshHostKeys` refused, both `sshHostKeys`/`SSH_HostKeys` casing, stopped/missing CLI
     errors, `TUN: true` vs `TUN: false` `ProxyCommand` formatting + `%` escaping + `${` refusal, atomic per-host
     private `known_hosts` write without touching user files).
   - Helm unit/integration tests for `SshAccessKind::Tailscale` argv construction (all 6 `CONNECTION_OVERRIDES`,
     `ControlMaster`, `UserKnownHostsFile`, `GlobalKnownHostsFile=/dev/null`, `HostName`, `--`,
     `-O check`/`-G`/`-O exit`), schema migration + historical fixture, `--ensure-hosts` `tailscale: true`,
     probe/provision/nested-Podman routing, and `POST /api/hosts/{id}/tailscale` releasing old ControlMasters before DB
     update (including graceful degradation when Tailscale is stopped).
   - GUI tests for Add host checkbox submission and Host settings switch toggle (including in-flight launch refusal and
     error recovery).
6. **Docs & spec updates:**
   - Update `SPEC.md` (host setup / SSH hosts / Tailscale host option and `known_hosts` guarantee) and `SPEC_impl.md`
     (SSH access kinds, `tailscale status --json` async preparation, per-host private `known_hosts` file, `ProxyCommand`
     for userspace networking).
   - Update website docs if remote SSH host pages explain adding SSH hosts / `--ensure-hosts` (`website/AGENTS.md`:
     incidental docs edits start no preview server).
7. **Changelog fragment & TODO removal:**
   - The stack carries a `kind: added` changelog fragment and the last PR removes the
     `Use the Tailscale SSH wrapper for Tailscale hosts` entry from `TODO.md`.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Mechanism:** System `ssh` + Farhelm-owned private `known_hosts` file populated from `tailscale status --json` +
  `tailscale nc` `ProxyCommand` when `TUN == false` (userspace networking). Confirmed explicitly that
  `~/.ssh/known_hosts` is never read or modified for Tailscale hosts.
- **Networking parity:** Full parity — MagicDNS node-name resolution + Tailscale host keys + automatic `tailscale nc`
  `ProxyCommand` when Tailscale runs in userspace networking.
- **GUI & config surface:** Checkbox in Add host (SSH tab) + switch in Host settings → Connection + optional
  `tailscale: true` in `--ensure-hosts` / `POST /api/hosts`.
- **Interactive SSH sign-in:** Keep interactive SSH sign-in enabled for Tailscale hosts (supports Tailscale hosts
  running ordinary `sshd` while being a no-op when Tailscale SSH authenticates automatically).
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** the maintainer wrote their own exact entry (the `## Release note` section).
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the request for a "Tailscale host" checkbox, the
clarification that `~/.ssh/known_hosts` must never be modified, and the decisions above.

**Binding repository constraints:**

- `SPEC.md` ("Security" — the 6 mandatory SSH `CONNECTION_OVERRIDES` and `--` option-injection guard) and `SPEC_impl.md`
  (SSH transport, two-master connection sharing, provisioning, leftover files).
- Helm database migration ladder and historical-schema fixtures in `crates/farhelm-helm/src/store.rs`.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- `.agents/validation.md` for GUI goldens (`check-gui-goldens.py` + `gui-goldens.py publish` when visual scenes change).
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and its "Instructions by path" for every path you touch.

**Planner proposals** (from research and the independent planning review):

- Use per-host/per-peer private `known_hosts` files under `<state_dir>/tailscale/` with atomic temp-file + rename and
  `0600` permissions (avoids shared-file read-modify-write locking and automatically replaces stale keys on rotation).
- Keep `HostKind::Ssh` unchanged (storing `tailscale: bool` on `HostRow`) and add `HostRow::ssh_access(&self)` so every
  existing SSH host capability, Podman place check, and host-identity flow continues working cleanly.
- Use a dedicated `POST /api/hosts/{id}/tailscale` verb route matching existing host boolean routes.

## Outline

1. `plan/tailscale-ssh-hosts/01-tailscale-ssh-access`: Tailscale CLI discovery, async `tailscale status --json` peer/key
   resolution, per-host atomic private `known_hosts` writer, `SshAccessKind::Tailscale` + resolved `TailscaleSshAccess`
   argv lowering (reusing `ssh::ssh_config_path_option` for `UserKnownHostsFile` and a dedicated formatter for
   `ProxyCommand` when `TUN == false`), and unit tests.
   `refactor: add Tailscale SSH access preparation and argv lowering`.
2. `plan/tailscale-ssh-hosts/02-helm-tailscale-hosts`: `helm.db` migration (`hosts.tailscale`) + historical fixture,
   `HostRow::ssh_access`, all six `SshAccess` call sites (`access.rs`, `transport.rs`, `ssh.rs`, `ssh_sign_in.rs`,
   `provisioning/setup_sign_in.rs`, `provisioning/service.rs`), `HostView` / `HostSpec` / `--ensure-hosts` / probe /
   provisioning / `POST /api/hosts/{id}/tailscale` (with `release_shared_connections` before DB update and graceful
   degradation when Tailscale is stopped), helm tests, and `SPEC.md`/`SPEC_impl.md` updates.
   `feat: support marking SSH hosts as Tailscale hosts in the helm`, with changelog fragment (`kind: added`).
3. `plan/tailscale-ssh-hosts/03-gui-tailscale-toggle`: Add host SSH checkbox, Host settings → Connection switch (with
   `super::launch_in_flight` guard), GUI tests, updated golden screenshots, incidental docs updates if needed, editing
   PR 2's changelog fragment if needed, and removing the TODO.md entry.
   `feat: add Tailscale host option to Add host and Host settings`.

## Fallbacks

- If a host's `destination` is an `~/.ssh/config` `Host` alias rather than a Tailscale node name, Tailscale peer lookup
  fails cleanly with a plain-English error telling the user to enter a Tailscale node name or Tailscale IP when
  `Tailscale host` is enabled (do not parse `~/.ssh/config` to map arbitrary aliases to Tailscale peers).
- If `scripts/gui-goldens.py publish` cannot reach the private goldens storage repository in the executor's environment,
  block with the exact error rather than committing a broken `goldens.txt` manifest.
- A reviewer finding asking to exec `tailscale ssh` directly (dropping `CONNECTION_OVERRIDES` or `ControlMaster`), to
  write into `~/.ssh/known_hosts`, or to disable interactive SSH sign-in is declined by decision; log it.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tailscale-ssh-hosts-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-tailscale-ssh-hosts-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes (`farhelm-helm` SSH/store/provisioning/hosts
tests, `farhelm-proto`, `farhelm-gui` host add/settings tests, `check-gui-goldens.py` for touched Add/Settings scenes
plus full `--update` before publishing if visuals changed, `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`). Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change. Never take a
state-changing action against the live Farhelm install on this machine (root `AGENTS.md`, "The live install is
off-limits").

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Use the Tailscale SSH wrapper for Tailscale
  hosts"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. A helm schema version that another plan
  also took is renumbered at rebase (`.agents/careful-rebase.md`).
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

Tailscale hosts are now explicitly supported - when adding a host you can mark it as a tailscale host, in which case
Farhelm uses tailscale to avoid known_hosts prompts.
