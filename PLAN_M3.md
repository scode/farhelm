# Farhelm M3: durability and resume

NOTE: This is the plan for milestone 3 only. It builds toward SPEC.md using the choices in SPEC_impl.md; where this
document and those disagree, they win. The overall motivation and the coarse milestone ladder live in PLAN.md; only the
current milestone is planned at this level of detail.

## Goal

Make sessions survive everything short of a host reboot, and make what a reboot does take away recoverable. SPEC.md's
durability section is the contract: clients, the helm, and the supervisor itself are all disposable — only the host
staying up matters — and after the one event that does lose terminals (a reboot), opening a session offers
restart-with-resume of the exact conversation it was running. This milestone also settles three debts the earlier
reviews parked here deliberately: an explicit atomicity policy for state-file writes with a fault-injection seam,
server-enforced create idempotency (the half of SPEC.md's double-submission guard M2 could not give), and the cgroup
hardening of process-tree stop.

M2 already proved sessions outlive a supervisor restart in the metadata sense; what is missing is truthfulness and
recovery. Today an exit during supervisor downtime and a host reboot produce the same shrug (exited, with whatever the
dead pane still happens to say, or unknown), an agent that never launched at all reports as exited rather than error,
and there is no way back into a conversation at all — the launch shim writes an exec-failure sentinel nothing reads, and
nothing captures which conversation a session's agent was running.

## User-visible outcome

- Restarting the supervisor (upgrade, crash, manual) is a non-event: live sessions keep running and reattach as if
  nothing happened. This already mostly works; M3 pins it as a contract rather than a happy accident.
- After a host reboot, sessions that were running show **interrupted** — an explicit lost-track state, not a guess about
  what happened — while sessions already exited keep their status and exit codes. Opening an interrupted session offers
  restart-with-resume; declining leaves it interrupted. Nothing ever respawns unattended.
- Restart resumes the same conversation (`claude --resume <id>`, `codex` equivalent) even when several sessions share a
  working directory — or says honestly that it never captured the conversation and offers the fallback resume or a fresh
  launch instead. Never a silently wrong conversation, never a `{conversation}` placeholder run unfilled.
- An agent that could not start at all (bad command, non-executable file) shows **error**, distinct from exited — the
  difference between "your invocation is broken" and "your agent ran and finished". A session the user stopped shows its
  stop annotation, distinct from an agent that exited on its own.
- A create that gets retried after an ambiguous failure (timeout, dropped connection) yields exactly one session.
- Stop gets stronger on Linux hosts with a systemd user manager (cgroup-scoped kill), and loses nothing anywhere else.

## Scope

### In

1. **One protocol bump to 5, all M3 wire vocabulary upfront.** M2.5 proved new tagged-enum variants are connection-fatal
   to older decoders, so nothing here is "additive if the encoding allows": `interrupted` and `error` status variants,
   the stop-annotation field, the `CreateSession` intent key and its key-reuse error, the optional snapshot-override
   fields (item 7), and the restart request/reply and resume-offer shapes all land in ONE proto PR that bumps
   `PROTOCOL_VERSION` to 5 — vocabulary first, handlers later, exactly how M2.5 shipped `DETACH_REASON_STALLED` before
   its sender. Golden and both-direction tolerance tests within 5; the version-skew handshake test grows the new
   boundary.
2. **Boot-id tracking, a durable last-known outcome, and interrupted classification — per session, never per server.**
   Classification needs ground to stand on that M2 deliberately never persisted: after a reboot erases tmux there is no
   liveness to probe, so "last known running" must be a stored fact. Each session gains a durable last-known-outcome
   record — launching (a durable generation committed BEFORE the external side effect, so a crash or reboot straddling a
   launch can never preserve a stale earlier outcome), running, or exited with code/annotation/error as observed —
   written at the transitions the supervisor actually witnesses (launch begun, launch confirmed, observed exit, stop,
   sentinel read). A launching generation whose side effects cannot be found on reload reconciles to error-or-retry
   territory, never silently to the previous run's outcome. The supervisor also stores the boot id it last saw alongside
   its host identity (`/proc/sys/kernel/random/boot_id`, equivalent elsewhere) and compares on reload. Same boot id:
   each session gets M2's per-row probing — a live pane continues untouched; a pane or server that died during
   supervisor downtime is **exited**, with the true code when the surviving dead pane still holds it and unknown code
   when nothing does (SPEC.md's wording — status section included — was sharpened by this plan to say exactly that:
   retained knowledge is not a guess). Different boot id: sessions whose last-known outcome was launching or running
   become **interrupted**, and the conversion is written durably IN THE SAME transaction that records the new boot id —
   a crash between the two would otherwise let the next startup take the same-boot path and misclassify the stragglers;
   a crash-boundary test pins it. Sessions already recorded exited keep their status, codes, and annotations. A database
   from before this milestone has no stored boot id: that first startup deliberately does NOT claim a reboot (no
   evidence either way — the no-guessing rule cuts both ways) and takes the same-boot path; the id is stored from then
   on. Interrupted is itself a durable outcome and persists until restart or delete — opening and declining resume
   changes nothing, and neither does another supervisor restart.
3. **error status via the launch shim's sentinel, with a per-launch lifecycle.** The shim already writes an errno-detail
   sentinel on exec failure (SPEC_impl.md records why the shim, not the shell, must write it); the supervisor now reads
   it and classifies **error**, with the sentinel's detail as the surfaced explanation. The sentinel is identified per
   LAUNCH, not per session — a stale sentinel from a failed first launch must not paint a later successful restart as
   error, so relaunch supersedes or clears it atomically (via item 5's write policy). Classification precedence is
   explicit: a current launch's sentinel outranks every inference. Exit codes alone never classify error — an agent that
   successfully execs and exits 126 or 127 is exited, and a test pins exactly that.
4. **Durable stop annotation, owned per launch.** SPEC.md makes stop annotations durable session metadata ("stopped by
   user" on an exited session) and nothing in the ladder owned them — found while planning this milestone. StopSession
   records the annotation in the store (part of item 2's last-known outcome); it rides the wire (item 1) and renders as
   its qualifier on exited sessions; it survives supervisor restarts and reboots. The annotation describes how the
   CURRENT run ended, so a successful restart clears it — and only a successful one: the clear commits with the new
   launch's durable generation, so a restart refused for a vanished cwd leaves the stopped outcome intact. Tests pin
   stop → restart → natural exit (no stale annotation) and stop → failed restart (annotation retained).
5. **State-file write atomicity policy plus a fault-injection seam.** One recorded policy — temp-write-then-rename, with
   fsync-before-rename and parent-directory-fsync-after where a torn or lost file would change classification —
   generalizing the atomic-private-write helper M2's snapshots added. The policy enumerates EVERY directly-written file
   class the supervisor and shim own and assigns each a tier: durability-bearing (the exec-failure sentinel — a lost one
   flips error to exited), best-effort atomic (alt-screen snapshots — losable, never torn), and regenerable or
   launch-transient (the generated tmux config — recreated from authoritative state, cleanup its only obligation), with
   a stated reason per tier. Per-launch spec files are NOT transient despite their lifetime: the shim learns the
   sentinel path from its spec, so a torn or missing spec converts a would-be error into a silent exited — they publish
   atomically before the tmux launch and a missing/unreadable spec is itself a classified launch failure. A reader may
   assume the complete old state or the complete new state at every failure point, never a truncated mixture; orphaned
   temps are cleaned on the next pass. The seam injects failures into the write, rename, and fsync windows AND verifies
   the sync ordering itself (file-sync before rename, directory-sync after), not just surviving content; SQLite's own
   durability is SQLite's.
6. **Server-enforced create idempotency, with a full reservation lifecycle.** A client-supplied intent key on
   `CreateSession`, deduplicated durably in the store and bound to a fingerprint of EVERY session-shaping request field
   — cwd, invocation, title, and item 7's kind and resume-template overrides (terminal dimensions are deliberately
   excluded: they shape the attachment, not the session): same key + same fingerprint replays the original outcome; same
   key + different fingerprint is its own error (a reused key is a client bug, not a merge), including an override-only
   mismatch; concurrent same-key requests collapse to one launch. The lifecycle is a real state machine, because a
   reservation alone cannot replay an outcome that never got recorded: reservation (pending, durable BEFORE any side
   effect, carrying the pre-assigned session and tmux identities so reconciliation knows exactly what side effects to
   look for) → outcome (the session id, or the original error). A retry that finds pending reconciles against reality —
   the reserved identities' side effects present means finish and record; absent means the crashed attempt never
   launched, so this retry performs the create under the same reservation. Every boundary (crash after reservation,
   during launch, before outcome commit) gets its own injection point — this is a create-lifecycle seam, distinct from
   item 5's file-write seam, which covers none of these windows. Outcomes tombstone rather than vanish: a replay whose
   session was since DELETED returns an explicit gone-error naming what happened — never a live-looking success carrying
   a dead id, and never a fresh duplicate. (These interactive-create keys are their own namespace; M7's spawn keys
   define their own lifetime when they land.) The UI generates one key per intended create and reuses it across retries
   of that intent.
7. **Minimal per-session integration snapshot** — the substrate restart needs that deferring profiles does not provide.
   At creation the supervisor records, immutably with the session: the launch invocation (M2 already does), the agent
   kind, and the resume invocation template. Kind defaults to recognition of the invocation's first token's basename
   (`claude`, `codex`, or none — recorded at create time, never re-guessed later), and the default template is built
   from the ORIGINAL first token, not a bare command name — a session launched as `/opt/bin/claude` must resume through
   `/opt/bin/claude --resume {conversation}`, and the codex default is `codex resume {conversation}` (the audited
   shape). Templates are stored structurally as argv vectors, not command strings, so a path with spaces survives
   without quoting heroics (a spaces-in-path test pins it); `{conversation}` substitutes into its own argv slot. Because
   basename recognition is honestly dumb — `env claude` or a wrapper script classifies as none — `CreateSession` also
   carries OPTIONAL explicit overrides for kind and resume template (wire fields in item 1; no new UI surface — the UI
   sends defaults, and the API/tests are the consumers until M5's profiles feed these same fields richer,
   user-controlled values). One validation invariant keeps the exact-conversation promise honest: a session with an
   INTEGRATED kind (claude/codex, derived or overridden) must have a template containing `{conversation}` — a
   placeholder-free template on an integrated kind is rejected at create, because once capture succeeded it could only
   ever discard the captured identity; placeholder-free templates belong to non-integrated kinds, where they are the
   future M5 fallback shape. This is SPEC.md's snapshot rule applied to the pieces M3 needs.
8. **Conversation-identity capture** for Claude Code and Codex, observation-only (no hooks, no agent configuration),
   behind the small `AgentKind` trait SPEC_impl.md sketches. The audited constraints bind: the on-disk record appears at
   first prompt submission (not launch), so correlation keys on first-input time with an unbounded gap tolerated; cwd
   munging is non-injective, so per-line JSON fields are the correlators; identity is claimed only when unambiguous —
   two near-simultaneous launches in one cwd stay uncaptured and take the fallback rather than a guess; plain resume
   appends under the same id (new ids only on explicit forks), so the watcher treats appends as the resume signal and
   re-verifies identity after each restart.

   **Known limitation, accepted in M3: a session whose own INVOCATION resumes a conversation is never captured.** Create
   a session with `claude --resume <id>` typed by hand and the agent appends to that conversation's existing record,
   whose header timestamp predates the session's capture window by however long ago the conversation started — so no
   candidate matches and the session correctly reports `FreshOnly`. It is a missed capture, never a wrong one, and it is
   deliberately not fixed here. The available fix is to treat an in-window APPEND to an out-of-window record as a match,
   and that trades directly against the guarantee this whole mechanism exists for: an append is evidenced only by a
   filesystem mtime, so any background write to an old record — the user resuming the same conversation in their own
   terminal, a vendor tool touching history — would become a capture, and a single such write is not something the
   ambiguity rule can see. The two clean fixes both belong later: item 9's restart already knows the identity it is
   resuming and carries it forward without re-discovery (so the common path to a resumed session is covered from M3
   onward), and M5's profiles can let an invocation DECLARE its conversation, which is authoritative rather than
   inferred. A fixture test pins the current behavior so the limitation cannot be lost.
9. **Restart with resume — the full SPEC contract.** Reuse the session's terminal when it still exists (prior run stays
   in scrollback); create a fresh one when it does not; reap leftover descendants of the prior run before relaunching,
   never alongside — including daemons left behind by an agent that exited on its own; restart on a still-running agent
   confirms, stops, then relaunches. A vanished working directory fails the restart with a clear error naming the
   directory and leaves the session intact. Resume fills the snapshot's template with the captured identity; no captured
   identity → restart says so and offers a FRESH LAUNCH, honestly labeled — not the launch invocation dressed up as a
   "fallback resume". (SPEC.md's verbatim-fallback-resume belongs to sessions whose profile defines a resume invocation
   without `{conversation}`; until M5 gives users that field, only an explicitly overridden placeholder-free template
   (item 7) can produce one, and the default path has nothing honest to offer but fresh.) A template referencing
   `{conversation}` is never run unfilled. No fresh-restart variant in v1. The environment is re-evaluated at each
   launch per the SSH-and-type contract.
10. **Process-tree hardening with cgroups.** `systemd-run --user --scope` wraps the launch where a Linux systemd user
    manager exists, making stop a cgroup kill with M2's /proc-plus-marker sweep retained as the backstop AFTER the
    cgroup kill (belt and suspenders, per SPEC_impl.md) and as the whole mechanism where no manager exists (CI). The
    selection is made per launch, recorded with the session, and survives supervisor restarts. macOS is NOT this
    fallback's territory: SPEC_impl.md defers the no-/proc macOS sweep to the Mac-supervisor work, and this milestone
    does not change that. Absence of a manager never degrades stop below M2's guarantees.

### Out (deliberately)

Rename (M5 — see the ladder change riding with this plan), archive (M7), status heuristics and profile CRUD (M5; item
7's snapshot fields are the interim substrate, not a profile system), live push (M5), helm-side persistence and
multi-host (M6), attachments and tabs (M4), spawn CLI, web auth, and provisioning/auto-start packaging (M7 — M3 makes
classification correct whenever the supervisor comes back, however it was started), the macOS process sweep (with the
Mac supervisor work, per SPEC_impl.md).

**macOS boot-id reading rides with that same Mac-supervisor work**, and is called out here so it has one recorded owner
rather than looking like an oversight in item 2. Item 2's detector is Linux's `/proc/sys/kernel/random/boot_id`; the Mac
equivalent (`kern.boottime` via sysctl) is deliberately not implemented in this milestone, for the same reason the
/proc-less process sweep is not. The behavior a Mac build gets meanwhile is the honest one item 2 already defines for a
host that publishes no boot id: the same-boot path runs forever and nothing is ever classified interrupted — never a
fabricated reboot claim, just a capability that host does not yet have.

## Testing decisions (settled while planning)

Capture is fixture-driven plus real checks: the fake agent grows modes that write claude-shaped and codex-shaped on-disk
records, so watcher and correlator logic — including every audited constraint — runs deterministically in CI. On top of
that, `#[ignore]`-marked tests drive the real agents end to end (launch, prompt, captured identity, restart,
conversation continues). BOTH agents get a real run before the milestone closes: claude headless, and codex headless if
it cooperates or through a documented interactive procedure if not — SPEC.md requires both integrations in v1, so an
unexercised codex path is a blocker, not a recordable gap. Interrupted classification needs no reboot: the boot id is
read through a seam, so tests inject a differing stored value. Atomicity tests use item 5's fault-injection seam — never
environment-variable tricks (repo rule). Environment re-evaluation is tested by rewriting a fixture HOME's rc file
between launches, never by mutating the test process's environment. The cgroup path runs where a user manager exists —
the development host — and is explicitly SKIPPED, loudly, where one does not (CI pins the fallback); the milestone is
not done until a documented run of the cgroup tests on a manager-equipped host is recorded.

## Order of work

Each step leaves something runnable; later steps only add. Tests ride with their step.

1. This plan, plus the PLAN.md ladder updates it implies (rename's home; stop annotations claimed here).
2. State-file atomicity policy and the fault-injection seam (the substrate later steps' tests stand on).
3. Proto: the complete M3 wire vocabulary, one bump to 5.
4. Boot-id tracking and interrupted classification, through helm and UI badge; durable stop annotation with it.
5. error status from the shim's sentinel, with the per-launch sentinel lifecycle.
6. Server-enforced create idempotency, store through UI retry behavior.
7. The per-session integration snapshot, then conversation capture behind `AgentKind` (fixtures first, the ignored
   real-agent tests with it).
8. Restart with resume, supervisor through helm API through UI (restart affordance; the interrupted-session resume
   offer), including terminal reuse, vanished-cwd failure, and the confirm-stop-relaunch path.
9. Cgroup hardening with fallback and backstop.

## Acceptance

M3 is done when all of the following hold, pinned by automated tests except where a documented manual run is named:

1. Kill -9 the supervisor mid-session and restart it: the session is listed, attachable, and its agent never noticed.
2. Same-boot classification is per-session: with the supervisor down, one session's agent exits and another's pane is
   killed outright; on restart the first is exited with the code the surviving dead pane retains (or unknown when
   nothing does), the second exited-unknown, and an untouched third continues live. Nothing is marked interrupted.
3. Simulated reboot (injected boot-id change): sessions whose durable last-known outcome was running list as
   interrupted; previously-exited ones keep status, codes, and stop annotations; interrupted survives open-and-decline
   and further supervisor restarts; nothing relaunches without the user confirming. A pre-M3 database (no stored boot
   id) takes the same-boot path on its first M3 startup. A session whose current launch left an exec-failure sentinel
   classifies error across that same reboot, never interrupted — the sentinel outranks the inference.
4. An invocation that cannot exec surfaces as error with the sentinel's detail; after a successful restart the error is
   gone; an invocation that execs and exits 126/127 is exited, never error.
5. A user-stopped session lists as exited with its stop annotation; the annotation survives supervisor restart and
   simulated reboot; restart clears it, and a natural exit after the restart carries no stale annotation.
6. Atomicity: for every enumerated state-file class, injected failures in the windows its TIER actually owes (write,
   rename, and fsync for durability-bearing; write and rename for best-effort atomic; atomic publication-before-launch
   and cleanup for launch specs and transients) leave a reader seeing the complete old or complete new state; the seam
   verifies the sync ordering (file before rename, directory after) rather than only surviving content; orphaned temps
   are cleaned; the sentinel and snapshot paths are exercised through the real seam.
7. Idempotency: the reply to a create is dropped AFTER the session durably exists; the retried create with the same key
   returns the same session id and no second process — including when the supervisor restarts between the two attempts.
   A failed create replays its original error; a same-key-different-fingerprint request errors, including a mismatch
   only in the kind/template overrides; concurrent same-key creates yield one session; a crash injected after
   reservation, during launch, and before outcome commit each reconciles on retry to exactly one session; a replay for a
   since-deleted session returns the explicit gone-error, never a dead id and never a duplicate.
8. Capture: two fixture-claude sessions in one cwd each capture their own identity and restart-resume their own
   conversation; the same for two fixture-codex sessions; a delayed first prompt (beyond any timeout in the code) is
   tolerated; a munged-cwd collision correlates through JSON fields; identity re-verifies across a resume append; two
   near-simultaneous launches stay uncaptured and offer fresh launch; a `{conversation}` template with no captured
   identity is never run — the offer is fresh launch (or an explicitly overridden placeholder-free template, where one
   was provided at create). Real-agent end-to-end: the ignored claude test run and its result recorded; codex run for
   real — headless or by the documented interactive procedure — before the milestone closes.
9. Restart: on a live session it confirms, stops the tree, and relaunches into the SAME terminal with prior scrollback
   above; on a terminal-less session it builds a fresh terminal; leftover daemons from an agent that exited on its own
   are reaped before the relaunch; a vanished cwd fails with an error naming the directory and the session survives; an
   rc-file change between launches is visible to the relaunched agent.
10. Cgroups: where a user manager exists (documented run on the development host), stop kills via the launch's own
    scope, the backstop sweep still runs after it, and the recorded selection survives supervisor restart; without a
    manager, every M2 stop test passes unchanged and CI proves it.
11. The full CI gate is green on every PR.

## Risks retired by this milestone

- The launch shim's sentinel finally has a reader — exec failures stop masquerading as exits, without ever reclassifying
  a real exit.
- "Interrupted" stops being inferable-only-by-the-user: the boot-id comparison makes the lost-track state explicit,
  per-session, and the no-guessing rule mechanical — including for stores that predate the feature.
- The create path's last silent-duplicate window (ambiguous transport failure, crash between commit and reply) closes
  with a durable, fingerprint-bound idempotency key.
- Conversation capture's hardest failure — silently resuming the WRONG conversation — is structurally excluded by the
  ambiguity bail-out, pinned by the near-simultaneous-launch and shared-cwd tests.
- The torn-state-file class of bugs becomes testable (and tested) instead of reasoned about.
- Restart cannot resurrect a session into a half-clean world: descendants are reaped first, and a missing working
  directory is a named error instead of a mystery launch failure.
