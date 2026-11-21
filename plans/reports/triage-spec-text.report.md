### What this was about

The triage decisions accepted the existing behavior for 35 findings where Farhelm's specs, guides or comments promised
something different or left out a limit. The plan put those decisions in the text so future fixes and reviews start from
the agreed behavior. All 35 outcomes were fixed as decided and moved to the outcome archive, with the PR for each
decision recorded there. The two PRs were delivered to the landing monitor as drafts.

### Things you should know

- After the main program exits, Stop, Delete and Replace can still end the background jobs Farhelm can identify as
  belonging to the session, even if the prompt only names extra tabs. Bash and Zsh use a Shell tab and offer Replace to
  start fresh after interruption. Their activity reading samples the bottom of the terminal, so it can miss activity
  elsewhere and the time to Idle can grow with the number of sessions on the host. The activity question before Suspend
  relies on that reading.
- If the supervisor crashes during terminal startup and the replacement misses the main terminal's socket, the program
  can run while the terminal stays unattachable until the next supervisor start. Restart retains scrollback color
  references, so earlier text can change color if the program changed the palette. A lost-output notice can clear when
  the supervisor re-adopts the terminal. Shift+Enter pairs the two bytes in its first write to the terminal device, and
  a short write can split delivery. Overline is among the rare terminal decorations Farhelm may omit. The link warning
  compares adjacent rows when they share a link identity. A cursor-positioned continuation can keep a separate identity.
  A user-started terminal paste can stall the window on macOS while the clipboard responds.
- Tensorlake's displayed suspension time and attribution are best effort. A failed operation, stale registry read or
  quit during reconnect or completion can leave the old Suspend time. Rediscovering the same pause can also move its
  noticed time. Orphan cleanup rechecks whether a host or recipe owns the sandbox name, then refuses a second concurrent
  cleanup of the same sandbox.
- Managed-checkout trust can put the host's canonical directory path in launch history. The first launch into a fresh
  checkout can write opted-in Claude trust even on a filesystem that lacks creation times. An older model name can stay
  selected after an agent switch when the destination agent's catalog offers it.
- Automatic host updates may start one at a time under bounded waits while other hosts remain usable. The helm can apply
  its upgrade floor only to a supervisor version it observed. A stopped supervisor may provide no version. Automatic SSH
  sign-in questions can open beside a visible control in the attention list, Hosts or the session card. Closing the app
  notifications panel marks the unread items in its last displayed list even if they were outside the viewport. Clear
  resolved uses that same list boundary.
- The helm's thirty-minute wait for an approved environment launch starts after Allow. The requesting command has a
  separate wait that starts when its request queues for the helm. An approved launch can still finish after a timeout. A
  retried create from an old terminal is refused. It keeps a fresh checkout that the earlier try may have allocated. A
  failed Restart can leave the session without a main terminal if ending the replacement host or sweeping its processes
  cannot be confirmed.
- The program's launch environment keeps the old version through a supervisor update. The terminal replies with the
  currently serving supervisor's version. Final conversation reports can still be matched to a retained process identity
  after the terminal host ends. Container sessions use the same launch path inside their own supervisor. The Rust
  integration tests start real terminal hosts from the built Farhelm executable, and the supervisor's glibc Linux unit
  tests start them from a fresh invocation of the test binary.
- The Custom commands and Agent wrappers guides now explain why forwarding Farhelm's Codex arguments keeps commands in
  the session even when conversation tracking is off.

### Open questions and possible follow-ups

The implementation comment above terminal Paste still calls the macOS wait a vendor defect. The triage decision
specified a change to the product spec. The comment can be refreshed when terminal Paste is next edited.

The Tensorlake and supervisor plans share two changed comment locations. Their reviews should keep the updated
conditions for a sandbox's noticed time and a final conversation report if the underlying behavior stays the same. The
overlap is recorded here for landing and needs no maintainer decision.

### PRs

- Recorded the 23 accepted behavior decisions in the specs. [#4467](https://github.com/scode/farhelm/pull/4467/changes)
- Corrected 12 outcomes across the specs, guides and comments.
  [#4471](https://github.com/scode/farhelm/pull/4471/changes)

#### Outcomes fixed in #4467

- `guarded-delete-replace-omits-surviving-shell-work.md`
- `tabs-only-stop-ends-surviving-shell-jobs.md`
- `shell-main-tab-spec-still-says-agent.md`
- `shell-recovery-spec-omits-replace.md`
- `shell-status-samples-visible-tail.md`
- `shell-idle-spec-omits-sampling-cadence.md`
- `late-bound-agent-host-misses-adoption.md`
- `canonical-folder-history-contract-conflict.md`
- `fresh-claude-trust-birthtime-contract-conflict.md`
- `automatic-update-planning-serializes-hosts.md`
- `host-restart-history-painter-omits-palette.md`
- `recovery-gap-notice-lifetime-contract-conflict.md`
- `stopped-host-update-floor-unresolved.md`
- `trailing-terminal-grid-lacks-sgr53-overline-after-old-route-retirement.md`
- `older-catalog-model-overrides-custom-owner.md`
- `auto-sign-in-uses-session-anchor.md`
- `notice-panel-marks-offscreen-rows.md`
- `macos-terminal-paste-blocks-gui-thread.md`
- `tensorlake-lifecycle-loses-running-evidence.md`
- `tensorlake-timestamp-clear-delayed-by-dial.md`
- `tensorlake-reprobe-moves-observed-time.md`
- `tensorlake-stale-registry-skips-timestamp-clear.md`
- `tensorlake-quit-rewrites-cleared-suspend-time.md`

The five Tensorlake outcomes use the same wording limitation under Tensorlake hosts in the implementation spec. The two
background-job outcomes share the confirmation and teardown rule in the product spec.

#### Outcomes fixed in #4471

- `joined-link-spec-omits-cursor-tail-exception.md`
- `trailing-environment-agent-launch-specs-disagree-receipt-relative-versus-post-allow-timeout.md`
- `trailing-spec-terminal-switch-point-names-deleted-terminal-type.md`
- `trailing-spec-shim-container-execution-branch-retired.md`
- `trailing-spec-report-discard-on-terminal-disappearance.md`
- `trailing-old-terminal-create-refusal-qualified-rollback-wording.md`
- `restart-unwind-omits-tree-sweep-failure.md`
- `trailing-testing-overview-falsely-generalizes-built-farhelm-host-program.md`
- `version-agreement-lifetime-doc.md`
- `trailing-shift-enter-unconditional-pty-write-guarantee.md`
- `orphan-cleanup-promises-unheld-provisioning-lock.md`
- `codex-wrapper-guides-omit-isolation-purpose.md`

### Checks

After rebasing both PRs, reran `dprint check` on the changed Markdown, `git diff --check` on each change and
`python3 scripts/check-agents-md.py`. All passed. The instruction checker reported one existing archive citation to a
missing heading. The feedback index still matches the files on disk. The 35 archived entries match the original
decisions except for their Execution fields, which now name their draft PRs.

The website build from `66bae22a` still applies after the rebase. The rebase preserved the website inputs byte for byte
and kept both plans' outcome records. For the two edited guides, `bun install --frozen-lockfile && bun run build` built
all 39 pages and validated all internal links. Astro reported its empty translation collection and missing content entry
for the 404 page.

Runtime tests were skipped because the changes are specification text, guides, and comments.

### Review gate

One fresh high-effort Sol review checked each PR against its recorded decisions, current implementation and exact queue
changes. The first review tightened the background-job consent rule, the stopped-supervisor update wording, the lifetime
of the missing-output notice, the Suspend activity question, and the rare main-terminal attachment gap. The second
review clarified the hyperlink comparison boundary, the Shift+Enter read split, fresh checkout rollback, best-effort
Restart cleanup and the wrapper troubleshooting advice. All ten findings were corrected. The rebase preserved both PRs'
reviewed and corrected source wording byte for byte. Both draft descriptions also passed a cold read.
