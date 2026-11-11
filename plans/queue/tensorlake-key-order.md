# Tensorlake: Farhelm's key is always the first one offered

Written against main at cb501937 on 2026-11-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Plan decisions to revisit"), verbatim:

> **Revisit accepting the agent-only SSH key fix for Tensorlake hosts.** Decided on the maintainer's behalf on
> 2026-11-10 when tensorlake-skip-ssh-agent blocked: Tensorlake snapshot builds and sandbox-host connections could offer
> a key from your SSH agent before Farhelm's registered key, which fails if Tensorlake's gateway rejects the first
> unknown key (that gateway behavior was never verified live). The plan stops consulting the SSH agent for the gateway,
> but a certificate or a hardware-token key configured in your own SSH configuration can still be offered before
> Farhelm's key, and OpenSSH 7.6, Farhelm's minimum, cannot switch the hardware-token one off. The plan was told to
> accept that, document both as exceptions (with the advice to leave those settings out of the configuration that
> applies to the Tensorlake host), and drop the promise that Farhelm's key is always offered first. The alternative was
> broader identity isolation guaranteeing Farhelm's key goes first in every supported SSH configuration, which needs a
> new design that keeps proxy and jump configuration and the OpenSSH 7.6 floor, and more than the plan's size limit.

The maintainer chose to close the gap. Research during planning found a fix far smaller than the "new design" the
decision assumed: two more `-o` options on Tensorlake connections only.

### What planning found (verify before relying on it)

From OpenSSH's source (7.6 tag and master, read through a summarizing fetch, not line by line) and the local OpenSSH
9.6's `ssh -G`; none of it was run against a real server:

- `pubkey_prepare` (`sshconnect2.c`) orders candidates: `CertificateFile` certificates, agent keys matching an
  `IdentityFile`, other agent keys (dropped by `IdentitiesOnly`), PKCS11 keys, remaining `IdentityFile` keys; a
  certificate is offered whether or not its private key exists. It then drops every candidate whose key type is not in
  `PubkeyAcceptedKeyTypes` (renamed `PubkeyAcceptedAlgorithms` in 8.5; the old name stays an alias), certificates and
  PKCS11 keys included. An ed25519 certificate's type is `ssh-ed25519-cert-v01@openssh.com`, so `ssh-ed25519` alone
  drops it.
- `IdentityFile` and `CertificateFile` accumulate and have no `none`; `PubkeyAcceptedKeyTypes` and `PKCS11Provider` are
  first-value-wins, and command-line `-o` is read before any config file, so Farhelm's values win over the user's.
- `PKCS11Provider=none` is a real off switch from OpenSSH 8.0. On 7.6-7.9 it is passed to `dlopen`, which fails with one
  `dlopen none failed` error line; nothing is fatal and no token keys load. Token keys are RSA-only on 7.6-7.9,
  RSA/ECDSA on 8.0-10.0, and ed25519 tokens arrive in 10.1, so the type filter alone does not cover 10.1 and later.
- A ProxyJump child gets `-F` but no `-o` options, so the user's jump and proxy configuration is untouched;
  `IdentityAgent=none` likewise leaves a jump hop its agent.
- Farhelm's Tensorlake key is ed25519 (`crates/farhelm-helm/src/tensorlake/ssh_key.rs`).

Together: Farhelm's `IdentityFile` heads the list, every certificate and every token key drops out, and the user's other
plain ed25519 key files may still follow Farhelm's. That makes Farhelm's key first, not only.

### Where things are (as of cb501937; correct line numbers to the truth as you go)

- `TensorlakeSshAccess::options()` in `crates/farhelm-helm/src/tensorlake/ssh.rs` (around 94-115; `IdentityFile`,
  `IdentitiesOnly=yes`, `IdentityAgent=none` around 104-106), `oneshot_options()` (around 129-133), and its pinning test
  (around 189-233). Other pinning tests: `crates/farhelm-helm/src/ssh.rs` around 1306-1330 and 1355-1415,
  `crates/farhelm-helm/src/transport.rs` around 1065-1103 and 1145-1146.
- Module comments stating the exceptions: `tensorlake/ssh.rs` around 22-33 (also "has not been verified for this
  change", which reads as change history), and `tensorlake/ssh_key.rs` around 10-11, which still says "a certificate
  with its matching private key", a qualifier the landing of tensorlake-skip-ssh-agent found wrong.
- Spec text: SPEC.md around 4317-4321 (the exceptions) and around 4386 ("Provisioning rides the user's existing SSH
  access — their keys, agent, and config", with no Tensorlake exception); SPEC_impl.md around 3399 (the same phrase),
  around 6676-6680 (the paragraph; "That gateway behavior was not verified live for this change" reads as change
  history) and around 6685-6690 (the option list, which leaves out `IdentityAgent=none`).
- The OpenSSH 7.6 floor: `ssh.rs` around 116-122 and SPEC_impl.md around 3404-3406.
- The earlier plan's changelog fragment: `releasing/changelog.d/tensorlake-ssh-agent.md`.

### Acceptance criteria

1. Every Tensorlake gateway connection (snapshot builds and preparation, host setup, credential delivery, host
   connections, one-shot commands) passes `PubkeyAcceptedKeyTypes=ssh-ed25519` and `PKCS11Provider=none`, from the one
   shared place the other Tensorlake options come from, on every supported OpenSSH version. Ordinary SSH hosts get
   nothing new (a test already checks that; keep it).
2. The key type in the option is tied to the key Farhelm generates, so changing one without the other fails a test.
3. Shown in a verification experiment, recorded in the working log and summarized in the report: with a user-style
   config that adds a `CertificateFile` and another `IdentityFile`, Farhelm's key is the first offered and the
   certificate never is.
4. The documented exceptions are gone, replaced by the precise promise (first, not only), in the module comments,
   SPEC.md and SPEC_impl.md; the leftover wording listed above is fixed.
5. The PR removes the TODO entry.

## Decisions already made

Settled by the maintainer on the 2026-11-11 planning page:

- **Direction:** restrict key types and turn off token providers for Tensorlake connections, and drop the documented
  exceptions. Rejected: keeping the decision with wording fixes only, and a Farhelm-generated ssh config.
- **Old OpenSSH:** pass `PKCS11Provider=none` on every version and accept the one stray error line on 7.6-7.9. No
  version detection.
- **Out of scope:** a user config that turns public-key login off for the Tensorlake host (`PubkeyAuthentication no`)
  still wins; that is a separate gap, for the report's follow-ups.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Size gate:** about 150 lines of diff (code, tests and docs, not counting the changelog fragment or the TODO.md
  removal). Over it, block and bring it back.

## Requirement sources

**The user's request:** the entry above and the decisions just listed.

**Binding repository constraints:** the OpenSSH 7.6 floor; proxy and jump configuration stays the user's (SPEC_impl.md
around 3399-3410, 6838-6841); SPEC.md and SPEC_impl.md as cited; root `AGENTS.md`, "Tests and validation", "Changelog
fragments", "Conventional Commits", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`. Nothing here may touch the
live install or a real Tensorlake account.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Use the spelling `PubkeyAcceptedKeyTypes`, which 7.6 needs and later versions accept as an alias; confirm the local
  OpenSSH prints no deprecation warning for it, and if it does, say what you chose and why.
- The experiment: a throwaway `sshd` in your scratch directory on a kernel-chosen port, with its own host key and an
  authorized-keys file; run the client with the Tensorlake options plus `-F` pointing at a test config that adds a
  `CertificateFile` (a certificate signed by a throwaway CA) and an extra `IdentityFile`, at `-vvv`, and read the
  "Offering public key" lines. Also `ssh -G` with `PKCS11Provider=none` and a provider configured. If `sshd` is missing
  or cannot run as this user, record that and rely on `ssh -G` plus the source reading, and say so in the report.
- The fragment: if `releasing/changelog.d/tensorlake-ssh-agent.md` has not shipped in a stable release (check
  CHANGELOG.md), amend it in place per root `AGENTS.md`, "Changelog fragments"; otherwise add a `fix` fragment.

## Outline

One PR: `plan/tensorlake-key-order/01-key-types`. `fix:`.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-helm`, the helm unit tests that pin SSH options and the Tensorlake and provisioning callers
(`cargo nextest run -p farhelm-helm --lib -E <filter>` through `python3 scripts/record-test-run.py`, with the pinned
tmux setup), and `python3 releasing/check-changelog.py format`. No cloud calls and no live gateway.

## Fallbacks

Examples of departures that need the scope reassessment: a generated ssh config or `-F`, extracting proxy settings with
`ssh -G`, version detection, or changing options for ordinary SSH hosts.

If the experiment shows a certificate or another key still offered before Farhelm's on the local OpenSSH, the promise
cannot be made as planned: do not drop the documented exceptions; block with the observation.

Decisions to log: the option spelling, the experiment's result, the fragment choice, and every review finding you
decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tensorlake-key-order-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/tensorlake-key-order/<nn>-<short-name>`,
  as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
