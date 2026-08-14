# Farhelm M7: the outer ring

NOTE: This is the plan for milestone 7 only. It builds toward SPEC.md using the choices in SPEC_impl.md; where this
document and those disagree, they win. The overall motivation and the coarse milestone ladder live in PLAN.md; only the
current milestone is planned at this level of detail.

## Goal

Close the ring around a product that already works: the edges nothing else could be built without, and the packaging
that lets someone other than its author install it. Five features and one release story — web-token auth and device
sessions, `farhelm spawn` and agent-spawned sessions, archive, host provisioning, the Mac app bundle — plus the two
items PLAN.md absorbed from M4's manual desktop pass.

What ties them together is that each one is an OUTER surface: the browser edge that has been unauthenticated on loopback
since M1, the in-session edge no agent can reach yet, the lifecycle verb that has been stubbed out since M1, the install
path that today is a paragraph of manual instructions, and the artifact that makes any of it reachable by a Mac user.
None of them changes how the core works; all of them change who can use it and from where.

This is not a polish milestone and not a refactor milestone. The architecture-assessment findings it carries (H6, U5,
U7) are each placed at the exact PR where their benefit stops being latent, and nowhere earlier — a rule this plan
inherits from M4.5 and M6.5 and does not relax.

## User-visible outcome

- The web UI asks for a token once per device and remembers it; `farhelm helm token show|rotate` is how the token is
  obtained, and rotating it logs every device out.
- A running agent can create sibling sessions itself: `farhelm spawn --cwd <dir>` inside any session prints a child
  session id and the child appears in every open client without a refresh.
- Sessions can be archived: hidden from the default list, everything in them shut down, metadata kept, and a restart
  brings one back with its conversation where the agent supports resume.
- Adding a host that has no supervisor offers to set one up, states exactly what it will do to the machine first, and
  does it over the user's own SSH with no root involved. The helm's own machine gets the same offer instead of M6's "run
  this command yourself" hint.
- There is a Mac app containing its helm and managed local supervisor, and Linux release artifacts that carry everything
  provisioning needs.

## Scope

### In

Items 1 through 8 are the milestone's PRs, in the order they land. Each item's acceptance criteria are stated with the
item; the milestone-level gate is the Acceptance section, which adds only what no single item owns.

1. **This plan, plus the PLAN.md header update it implies** (M6.75 moves to history, the M7 ladder entry gains its
   pointer). Nothing else changes.

   Acceptance: `dprint check` passes; PLAN.md's note names PLAN_M7.md as the current milestone.

2. **Proto v11 — all M7 wire vocabulary in one PR.** The standing rule since version 5: wire shapes never trickle, and
   one milestone's vocabulary rides one bump rather than a version per change. The bump carries spawn and archive
   together; item 3's web token is deliberately NOT in it, because the browser edge has no hello and no wire types at
   all — it is gated on the build stamp instead (SPEC_impl's "Version and skew"), which is a different handshake
   answering to a different rule.

   **Spawn's admission shape.** SPEC.md puts a per-session credential in the session's environment and has the spawn CLI
   reach "its own supervisor over local IPC only". So the credential has to travel at connection setup, and `Hello` is
   the only message that runs there. Position: `Hello` gains `auth: Option<SessionAuth>` carrying the session id and the
   token, and the supervisor's admission rule keys on that field's PRESENCE — never on `role`, which this crate
   documents as diagnostic free text and which must not quietly become an authorization input. A hello with no `auth` is
   a full-authority local peer, exactly as today: reaching a 0600 socket inside a 0700 directory already proves the
   ability to run commands as the user, so there is nothing for a credential to add there. A hello WITH `auth` is a
   RESTRICTED peer and must validate.

   Optional does not mean accidentally bypassable. The credential is ATTRIBUTION — trustworthy parentage — plus
   deliberate self-scoping: a spawned agent's own tooling opts into the narrow slice, which protects against accidents
   and confused-deputy shapes. It is not and cannot be a same-uid security boundary. Every process inside a session runs
   as the user who owns the 0600 socket, so mandatory proof would break every existing local client while stopping
   nothing; a session process that deliberately omits `auth` is the user acting on their own machine, outside SPEC.md's
   single-user threat model.

   That restriction is what earns the bump under this file's own sharper rule (version 9's): a field whose omission
   changes what the receiver does is not additive whatever serde makes of it — and the direction here is the dangerous
   one. A v10 supervisor ignoring `auth` would grant a session peer FULL authority rather than the narrow slice it asked
   to be held to. The hello refusal is what keeps that from ever happening.

   **The rest of spawn's vocabulary.** `CreateSession` gains `parent: Option<String>` and `SessionInfo` gains the
   matching field, both defaulted-absent. It also gains `profile_name: Option<String>`, because v10's actual create
   shape accepts either a raw `invocation` or a `profile_id`, while spawn's `--agent` accepts the human-facing profile
   NAME. In v11 exactly one launch selector is present: `invocation`, `profile_id`, or `profile_name`. The supervisor
   resolves a name inside the create operation, before reserving or launching anything. That keeps lookup and creation
   atomic, gives a restricted peer no catalog-read authority, and removes the list-then-create TOCTOU. No match or an
   ambiguous name is `InvalidRequest`, with the candidates named; the chosen selector and value join the idempotency
   fingerprint. `parent` joins that fingerprint too: replaying a key with a different parent is the existing `Conflict`,
   never a silent return of the first child.

   Amended 2026-08-07, during item 4's review: "exactly one selector" as originally written contradicted SPEC.md's "only
   `--cwd` is required" — a spawn with no `--agent` sends NO selector and means "the host's last-used profile". The rule
   as shipped: a full-authority create still requires exactly one selector (a selectorless request is refused, as
   always); a SESSION-AUTHENTICATED create may omit the selector, and that omission means derive-the-last-used-default,
   refused as a precondition when none resolves. The exception is scoped to the one connection class whose whole purpose
   is SPEC.md's defaulting contract, and both cases are pinned by test.

   Spawn reuses `CreateSession` outright rather than earning a message of its own: SPEC.md says everything but `--cwd`
   "defaults exactly as interactive creation does", and a second creation message would be a second place for those
   defaults to drift. `intent_key` is likewise reused for `--idempotency-key` — one deduplication mechanism, not two key
   namespaces answering "did this intent already happen" differently. `ErrorKind` gains `Unauthorized` (a new tagged
   variant, independently bump-earning) for a bad credential and for a session peer reaching past its permitted
   operation.

   **Idempotency-key lifetime, settled here as spawn-time policy.** The supervisor's `create_reservations` machinery
   (M3) already fits: it is a claim table with tombstones that outlive their session on purpose, so a replay after a
   delete answers "yes, and it was deleted" rather than creating a duplicate. SPEC.md bounds spawn's keys differently —
   they "live as long as the child session does" — and that is a real conflict, not a gap. Position: the reservation ROW
   keeps being the single mechanism; what becomes policy is the DEDUP WINDOW, recorded per row as a scope. Two scopes
   exist: `permanent` (interactive creates, M3's behavior unchanged) and `session_lifetime` (spawn), where the row stops
   deduplicating and becomes prunable once its child session is gone. Spawn takes the bounded scope because SPEC.md says
   so and because the alternative is worse on its own terms: an agent-authored key retained past its child's deletion is
   a key that can never be used again, permanently, on a surface agents script against. Interactive creates keep the
   permanent scope because nothing in SPEC.md bounds them and M3's argument — a double-submit retry must never duplicate
   — is about a window measured in seconds where the row is cheap. The scope is DERIVED from the connection that created
   the reservation, never declared on the wire, so nothing a caller sends can widen its own dedup window.

   **Archive's vocabulary.** `SessionInfo` gains `archived: bool` (defaulted false) and `ControlMsg` gains
   `ArchiveSession`/`SessionArchived`. Three things deliberately do NOT appear. There is no confirmation flag: archive
   mirrors `DeleteSession`, which carries none, because SPEC.md's confirmation is a client obligation and a wire flag
   would let a client skip it invisibly. There is no `UnarchiveSession`: SPEC.md's lifecycle list has no such verb and
   says restart on an archived session unarchives it, so inventing one would be inventing product behavior. And there is
   no archived STATUS — SPEC.md enumerates the statuses and archived is not among them; it names archived as a durable
   metadata FLAG, which is what this is.

   **`ListSessions` grows nothing.** M6.75 kept `ListQuery` cursor-and-limit only, on the rule that it grows nothing
   until a consumer needs it, and archive does not make one. The helm's drain must keep pulling each supervisor's
   COMPLETE list — a drain that hid archived rows would corrupt the cache's whole-fleet completeness for exactly the
   reason M6.75 gave about filtered drains — so `archived` simply travels on `SessionInfo` and the HELM's merged view
   filters. Filtering stays where M6.75 put it, in one place.

   Acceptance: golden encode/decode cases for every new field and variant, including all three create-selector fields
   and their present/null shapes; tolerance tests in both directions (a v10 decoder fed a v11 `Unauthorized` error or an
   `ArchiveSession` fails, and is pinned failing); the skew boundary and `protocol_version_is_pinned_at_11` move to 11;
   nothing in this PR serves any of it.

3. **Web-token auth and device sessions.** Early on purpose: every later PR's e2e runs authenticated, so no surface in
   this milestone is ever built against an open helm and retrofitted.

   **First commit: the `api_router`/`build_router` split** (assessment finding H6). Today `build_router` is one chain
   that layers `require_loopback_origin` and `stamp_build` over everything including the static `ServeDir` fallback.
   Auth is the first requirement that must cover `/api/**` and both WebSocket routes while deliberately NOT covering the
   UI bundle — the browser needs a page to type the token INTO — so the split stops being taste and becomes what makes
   the auth boundary structural instead of a per-route discipline. It lands as its own commit, a checked no-op, before
   any auth code, per M4.5's rule that functional no-ops never interleave with functional changes.

   **helm.db tables**, as schema step 7 in the existing `PRAGMA user_version` ladder with a
   `- 7: PLAN_M7.md item 3 — web-token authentication and device sessions.` entry in `apply_schema`'s version history,
   matching the convention exactly. `web_token`: one row, the recoverable token and its creation time.
   `device_sessions`: the cookie's hash as primary key and its creation time. `last_seen` does not exist: updating every
   authenticated request for a value nothing reads is write amplification with no v1 purpose. `created_at` stays on both
   tables even though application code reads neither today. It is written once at mint, and a credential table with no
   timestamps is undiagnosable when something goes wrong.

   **Credential storage, including a real specification conflict.** SPEC.md:430-432 requires the user to VIEW the web
   token through the app or `farhelm helm token show`; hash-only storage cannot implement that. The token is therefore
   stored recoverably (plaintext) in helm.db. This accepts the same exposure as spawn credentials: anything that can
   read helm.db already runs as the user, and the state directory is 0700. SPEC_impl.md:644 instead says the token is
   "stored hashed". That is a genuine SPEC.md-versus-SPEC_impl.md conflict, resolved here in favor of SPEC.md's viewable
   token; the auth PR amends SPEC_impl.md under its standing sync rule rather than papering over the disagreement.

   **The user-facing token surface.** V1 deliberately satisfies SPEC.md:430-433's "the app UI, or" disjunction through
   `farhelm helm token show|rotate`, not an in-app settings surface. Linux users run the installed CLI; item 8 documents
   the Mac bundle's internal CLI path. This keeps token management available on the helm's own machine without adding a
   second UI flow to the close-out milestone. An in-app affordance is post-v1 in Out, not silently assumed here.

   Device cookies are never redisplayed and stay hash-only. Plain single-round SHA-256, not a password KDF: KDFs exist
   to make LOW-entropy secrets expensive to guess, while each cookie is a uniform random 128-bit value. A KDF here buys
   a dependency and per-request CPU against a threat that does not exist. Comparison is constant-time.

   **Cookie exchange.** `POST /api/auth/token` with the token in the BODY, never a query string (query strings land in
   logs and in `Referer`). The helm mints a device-session value, stores its hash, and returns it as an HttpOnly,
   SameSite=Strict, Path=/ cookie with an explicit Max-Age of 400 days (the browser ceiling). It is persistent because
   SPEC.md says the browser asks once per DEVICE, not once per browser process; a named Playwright test pins reuse after
   a browser restart. Rotation is the only expiry mechanism in v1 — there is no idle or absolute server-side expiry.
   Deliberately NOT `Secure`: SPEC.md binds loopback plain HTTP in v1 and calls localhost a secure context, so a
   `Secure` cookie would simply never be sent in the only deployment that exists. That sentence is here so nobody later
   "fixes" it.

   Why a cookie at all, given that a cookie is what makes CSRF possible where a bearer header would not: the browser's
   WebSocket API cannot set request headers, and both the terminal socket and M6.75's fleet feed are WebSockets. A
   cookie is the only credential a browser can attach to a WS handshake without inventing an in-band auth message or
   putting a token in a URL. `require_loopback_origin` therefore STAYS, alongside SameSite=Strict — two overlapping CSRF
   defenses whose combined cost is nil.

   Amended 2026-08-07, during item 3's review: the two paragraphs above are superseded — review found the cookie design
   defeats the token's own purpose. Browser cookies are scoped by HOST, not port, so any process on another loopback
   port (which is to say: any other local user) can receive the device cookie from a lured browser and replay it with a
   forged Origin. And the premise that a cookie is the only WS-attachable credential was wrong: the browser can set
   `Sec-WebSocket-Protocol`. The shipped design: the exchange returns the device credential in the response body; the
   browser keeps it in localStorage (origin-scoped INCLUDING the port); REST carries it in an `Authorization` header
   through the one client funnel, and the WebSockets carry it in `Sec-WebSocket-Protocol`, validated before upgrade.
   `device_sessions` still stores only the hash. No ambient credential remains, so the CSRF surface disappears and the
   origin check becomes defense in depth. HttpOnly's XSS shielding is knowingly given up: an XSS attacker inside this
   origin can already drive the API directly, so it bought nothing real, while port scoping buys the exact boundary
   SPEC.md claims for the token. Once-per-device persistence is localStorage's own behavior; the browser-restart test
   stays, adapted.

   **Enforcement.** One middleware over the api router. REST answers 401 with a structured JSON body so the UI can tell
   "not authenticated" from any other failure. WebSocket routes are rejected BEFORE upgrade with the same 401, since the
   cookie rides the handshake — an accepted-then-closed socket would hand the client a close code to interpret instead
   of a status. `stamp_build` stays OUTERMOST so a 401 still carries the build stamp: a skewed bundle must be able to
   tell it is skewed rather than reading 401 as "wrong token". Exactly two routes are unauthenticated: the static bundle
   and `POST /api/auth/token`.

   Rotation does more than make the NEXT handshake fail. In the same operation that writes the new token and deletes
   every device-session row in one transaction, the helm closes every active authenticated WebSocket server-side — both
   fleet feeds and terminals. Deleting every row and closing every live authenticated socket is the complete meaning of
   SPEC.md:432-433's "invalidates every device session".

   Amended 2026-08-07, during item 3's build: this paragraph originally said the helm "is one process and already knows
   its sockets", so no coordination machinery would be needed. That was wrong about the process boundary —
   `farhelm helm
   token rotate` is its own CLI invocation, and the sockets live in the SERVING helm. The shipped shape
   is a private token-control Unix socket in the helm state directory (0700, the same trust boundary as reading helm.db,
   carrying exactly one command): the CLI rotates through the serving helm when one owns the directory, and falls back
   to a direct offline database transaction when none does — which is also what keeps `rotate` usable on a stopped helm.
   An auth epoch remains rejected; the socket is the narrow bridge to the one process that can close what must close.

   **What the threat model becomes.** Until this PR the helm's boundary is loopback-only: the bind plus the origin
   check, with any local process or user able to drive the whole API. After it, the BIND is unchanged (SPEC.md: refuses
   non-loopback in v1, TLS post-v1) and the token adds exactly what SPEC.md claims for it — it keeps other local
   processes and users out. What it deliberately does not do: it is not transport security (the wire is plain HTTP; a
   user-managed SSH forward is what protects it off-machine), it does not identify WHICH user (single-user product), and
   it defends against nothing that can already read the helm's state directory or the browser's cookie jar, because that
   is an attacker already running as the user.

   **UI.** The load-bearing change is one response-classification point: `send_within` becomes the 401 funnel, returning
   a typed error that separates `Unauthenticated` from an ordinary failure, so the token prompt is raised centrally
   rather than by twenty-two call sites each formatting a status. Whether the auth PR shares a `reqwest` client OBJECT
   or keeps the current construction shape is an implementation detail decided in front of the real code, not a plan
   contract. `read_failure` and `refusal_text` keep their existing jobs for everything else. The prompt itself is a
   full-page in-page surface (never a browser dialog — SPEC_impl's wry note binds here) that exchanges the pasted token
   and re-reads. Skew and 401 are distinct surfaces and the skew prompt wins when both apply, since a skewed bundle's
   reading of any response is suspect.

   **Desktop.** This PR builds the exchange primitive the native app will use. Deliberately not an "is this the embedded
   app" bypass: one enforcement path that the app satisfies like any other client is the shape that does not rot. The
   current desktop renderer is still the thin client documented in `crates/farhelm-ui/src/main.rs`; item 8 supplies the
   embedded helm, managed local supervisor, and the two client-stack exchanges its separate cookie jars require, so the
   path is exercised end to end only once that item lands.

   **e2e harness migration.** `start-stack.sh` and the Playwright setup obtain the token via `farhelm helm token show`
   against the harness state dir and perform the exchange once into a shared `storageState`, so every spec from here on
   runs authenticated without per-test ceremony. `rest_harness.rs` grows an authenticated default plus an explicit
   unauthenticated mode, so the Rust helm tests do not each hand-roll it.

   Acceptance: an unauthenticated REST call and an unauthenticated WS upgrade both 401 (the WS before upgrade); the
   static bundle and the token-exchange route do not; a correct token exchanges once and every later read succeeds from
   the cookie alone, including after a browser restart; `farhelm helm token show` returns the minted token after a helm
   restart; `farhelm helm token rotate` makes every existing device session's next read 401 and re-prompt AND drops an
   already-open feed socket and an already-open terminal socket, pinned as named Playwright tests because they make
   SPEC.md's rotation sentence executable; a 401 still carries the build stamp; the whole e2e suite runs authenticated.

4. **Supervisor spawn and the `farhelm spawn` CLI.** Before archive because archive's UI work sits on the same row and
   filter surfaces, and M6.75 deferred the parent-reference filter to "M7, beside the feature that mints parent
   references" — landing spawn first means that surface grows its parent filter and its include-archived toggle in one
   direction rather than being reopened in reverse.

   **Injected environment.** `launch.rs` already owns `FARHELM_SESSION_ID`. It gains `FARHELM_SESSION_TOKEN` (the
   per-session credential) and `FARHELM_SUPERVISOR_SOCK` (the socket path). SPEC.md guarantees only the session id and
   the credential as contract and calls other Farhelm variables illustrative — so the socket path is injected for the
   CLI's own use and is not promised to third-party scripts. It is REQUIRED for spawn: absence is a precondition failure
   naming `FARHELM_SUPERVISOR_SOCK`, with nothing dialed. A default-state-dir fallback can only dial the WRONG
   supervisor when the session belongs to one started with `--state-dir`.

   **Credential storage and lifetime.** Minted per SESSION at creation and stored as PLAINTEXT in the session's row.
   Hashing is not an option here and the reason is structural: a restart has to re-inject the same credential, so the
   supervisor must be able to produce it. The exposure this accepts is identical to one SPEC_impl already accepts —
   launch specs in the same 0700 directory carry the user's own credentials on the agent's command line — and anything
   that can read supervisor.db already runs as the user. The credential is per session, not per launch: SPEC.md scopes
   it to "that one session" and says it dies with it, and re-minting on restart would silently invalidate a token a
   descendant captured before the restart while changing nothing about who is inside the session. It is deleted with the
   row. One discipline rides with it: nothing may log a session's environment, and the sweep's environ scan (which
   already matches an exact `FARHELM_SESSION_ID=` prefix and logs no contents) is pinned that way by test.

   **Upgrade semantics.** The supervisor schema migration mints a credential for EVERY existing session row in the same
   migration transaction; upgraded storage never contains a credential-less row. A running pre-upgrade session cannot
   receive it because process environments are immutable. Its next restart injects the minted credential and the socket
   path. Until then, `farhelm spawn` fails as a precondition with no connection and a message saying the session
   predates spawn support and must be restarted. This is an upgrade edge, not a reason to weaken admission or guess a
   socket.

   **What a session peer may do.** Exactly one thing: `CreateSession`. Anything else on an authenticated-as-session
   connection is `Unauthorized`. If the request sets `parent`, the value must equal the peer's own authenticated session
   id — parentage is organizational metadata, but a forged one is a lie that shows up in every client's filter, and
   there is no legitimate caller that needs to attribute a child to someone else's session.

   **Defaults, and the one place SPEC.md's wording meets an implementation conflict.** SPEC.md says spawn's non-`cwd`
   arguments default "exactly as interactive creation does (last-used profile on the host, generated title)". But M6.75
   put the remembered default in helm.db, on the argument that only the component which can ASK a user should own
   defaulting policy — and spawn never touches the helm. Position: the supervisor does not grow a second policy store.
   It DERIVES THE last-used profile from the sessions table: the most recent session with a source profile, with no
   existence filter. If that profile no longer exists, spawn's honest equivalent of asking is REFUSING the request as a
   precondition, nonzero, naming `--agent`, with no session created. It never walks backward to an older surviving
   profile; that would be precisely the guessing SPEC.md's creation rule forbids.

   The supervisor's derivation keeps offline spawn independent of the helm, but SPEC.md:151-152 still defines ONE
   last-used profile per host. The helm therefore converges its remembered default from the drain: when a profile-backed
   session has source-profile provenance newer than the remembered default's provenance, the helm updates the default to
   match. Every profile-backed create, including `spawn --agent B`, becomes that host's last-used profile within one
   drain interval. This does not change the dialog's consumed-once-per-activation rule; a background update changes only
   what a FUTURE dialog seeds, which is the point. `--agent` sends the profile NAME in item 2's `profile_name` field —
   ids are opaque UUIDs nobody would type — and the supervisor resolves it atomically inside `CreateSession`. An
   ambiguous name or no match is refused with the candidates named rather than resolved by a rule nobody asked for.

   `--parent` is NOT defaulted to the caller's own session. SPEC.md's example passes it explicitly, and "everything else
   defaults exactly as interactive creation does" is a statement about a creation path that has no parent at all. A
   spawn without `--parent` produces a parentless child, which is a legitimate thing to want.

   **The scripting contract** (SPEC.md's, restated because this PR is where it becomes real): on success the child
   session id goes to stdout and the process exits zero, and success means the session EXISTS — a child whose agent then
   fails to launch still prints its id and exits zero, carrying `error` or `exited` as its own status. Precondition
   failures exit nonzero with a message on stderr and NOTHING on stdout. Only `--cwd` is required. Stdout hygiene is a
   hard rule here for a reason SPEC_impl already states elsewhere about the stdio proxy: agents will write
   `$(farhelm spawn …)`, so stdout carries the id and a newline and never anything else, with all diagnostics on stderr.
   One nonzero exit code, not a taxonomy — a code space is a compatibility surface owed forever, and the consumer is an
   agent reading text. Deliberate reading of SPEC.md:141-143: its absolute-path rule binds the STORED/WIRE value, which
   stays absolute-only. The spawn CLI canonicalizes a relative argument against its own real working directory before
   sending it, so the wire satisfies that rule by construction. This is input ergonomics on the one creation surface
   that genuinely has a cwd, not a relaxation of the requirement.

   **Idempotency in practice.** `--idempotency-key` becomes `intent_key` with the `session_lifetime` scope from item 2.
   "Scoped to the host" is automatic — the supervisor IS the host. A repeat with the same key returns the existing
   child's id and exits zero; a repeat with the same key and a different fingerprint is the existing `Conflict`, nonzero
   with stderr.

   **Visibility.** Children appear everywhere through M6.75's feed with no new mechanism: the helm's drain picks up the
   new session, the cache write changes rows, and FleetEvents bumps. The honest bound is one drain interval (~3s), which
   is the same bound M6.75 accepted for status and which satisfies SPEC.md's "without refresh" — a statement about the
   absence of user action, not about latency. See the review questions.

   **Parent filter.** The filter M6.75 deferred lands across the same three surfaces as its existing five dimensions.
   `GET /api/sessions` gains an optional `parent=<session-id>` query parameter; the helm's merged-view `SessionFilter`
   applies an exact parent-id predicate before pagination to both persisted and live rows; and the UI adds a parent
   control beside host, directory, profile, status, and title. It filters the flat list to direct children only — no
   hierarchy, transitive walk, or supervisor-side filter — and participates in the cursor fingerprint and matching count
   exactly like the existing dimensions.

   **The real-agent acceptance leg.** The always-on fake-agent browser test proves the deterministic product path, but
   it does not carry SPEC acceptance test 7's real-agent contract. A second Playwright leg reuses M6.5's helper and is
   gated by `FARHELM_REAL_AGENT=1`, with the same visible skip line when ungated. It asks real Claude to create a new
   workspace with `jj workspace`, invoke `farhelm spawn` for it, and verifies the child appears in the open client
   without refresh. Vendor credentials and network keep this out of CI; the named test makes the full contract one
   deliberate command rather than an undocumented manual improvisation.

   Acceptance: a fake agent inside a session runs `farhelm spawn --cwd <dir>` and the child appears in an open browser
   with no refresh and no navigation; the env-gated real-agent leg creates a `jj workspace`, spawns into it, and
   observes the same no-refresh appearance, while skipping loudly when ungated; the CLI's contract is pinned for stdout
   shape, exit codes, and `--cwd`-only invocation; a migrated pre-upgrade row has a credential, its still-running old
   process is refused with the restart remedy, and restart makes spawn succeed; absence of `FARHELM_SUPERVISOR_SOCK`
   names that variable and dials nothing; an ordinary local peer omitting `Hello.auth` keeps today's full authority; a
   session-authenticated peer issuing anything but `CreateSession` is refused; a `parent` naming another session is
   refused; requests naming zero or multiple launch selectors are refused; `profile_name` is resolved inside create,
   with no-match and ambiguity refused and candidates named; a repeated idempotency key returns the same child and
   creates nothing, while replaying it with another parent is `Conflict`; the key stops deduplicating once its child is
   deleted; a spawn with no source profile fails as a precondition with no session left behind; a deleted last-used
   profile refuses with `--agent` named rather than falling back to an older surviving profile; the helm drain advances
   the host's remembered default when a newer profile-backed session appears without changing an already-open dialog;
   the REST parent query, merged-view predicate before pagination, and UI control all select exactly the direct
   children.

5. **Archive.** The lifecycle verb stubbed out since M1, in the supervisor, the helm, and the UI.

   **Supervisor semantics.** An `archived` column via the schema ladder, and an archive path that tears the session down
   completely — agent AND tabs — then keeps the row. `teardown.rs`'s module doc already anticipates this and says a
   future archive path should state what IT does rather than inherit delete's comments, which this PR does. So: archive
   reaps the agent and tabs the way delete does, while metadata AND attachments survive and the archived flag is set.
   The terminal is GONE, because the tmux session is killed — that is SPEC.md's rule, and it is what makes
   restart-on-archived create a fresh terminal rather than reuse one. Attaching to an archived session is refused with
   `InvalidRequest` naming the state, not `NotFound`: the session exists, and telling a client it does not would send it
   to the wrong remedy. Archiving an already-archived session is an idempotent no-op returning the current `SessionInfo`
   — it is the state the caller asked for, and an error there would make a retry after an ambiguous transport failure
   look like a bug.

   Archive is NOT delete's teardown minus the row drop. It preserves the session's attachments directory too: SPEC.md
   ties attachment removal to deletion, and a resumed conversation may still name those paths. Removing attachments
   remains delete's job alone.

   The durable outcome archive records is `Exited` with the existing `STOP_ANNOTATION`. No new annotation string:
   annotations are prose a client renders verbatim and never branches on (the field's own docs), and the archived flag
   is already what tells the user why the session is where it is. Archive never yields `Interrupted` — that is the
   boot-id lost-track state, and an archive is a deliberate user action about which nothing was lost.

   **Restart unarchives and resumes.** Restart on an archived session clears the flag, creates a fresh terminal, and
   resumes the conversation wherever `RestartOffer` says it can — no new offer variant, no new path. Tabs do not come
   back: SPEC.md is explicit that after an archive tabs are gone and nothing recreates them.

   **Helm.** `POST /api/sessions/{id}/archive`, recording the reply's fresh `SessionInfo` before answering under the
   existing every-mutation-records rule, which also publishes the FleetEvents bump under the changed-only rule — no new
   mechanism. The merged view's filter gains a boolean `include_archived` toggle, default OFF. It runs through the same
   merged-view predicate machinery as M6.75's five filter dimensions, but it is semantically only a default-off
   inclusion switch, not a sixth multi-valued dimension. SPEC.md leaves the mechanism open (it says archived sessions
   are hidden from the DEFAULT list, and a default implies a non-default); a structural hide would need a second escape
   hatch invented to ever see them again.

   **UI, and where assessment finding U7 lands.** `SessionRow` is at fifteen props and archive adds more — an archive
   handler, its confirmation state, the archived flag. That is where the RowActions/RowState grouping stops being
   cosmetic, so it lands HERE and as the FIRST commit of this PR: group into a `RowState` (derived display state) and a
   `RowActions` (callbacks), checked as a no-op by the existing suite, before any archive prop is added. `HostRow`'s
   eighteen props are deliberately left alone — U7's benefit is where props are growing, and nothing in this PR grows
   that one.

   Amended 2026-08-07, during this item's review: `RowActions` does not survive contact with Dioxus. Eleven reviewers
   independently proved that nesting freshly constructed `EventHandler`s inside a props struct bypasses the framework's
   callback-prop memoization — every list render retained fresh handlers and rerendered every row, which is precisely
   the non-no-op the grouping was checked against but the test suite cannot see. The shipped shape: only `RowState`
   (derived display state) is grouped; callbacks remain DIRECT props backed by stable `use_callback` handles created
   outside the row loop, and a repeated-refresh render-count regression pins the behavior. U7's growth argument still
   holds for state — archive's flag and confirmation land as `RowState` fields — while its callback half is withdrawn as
   incompatible with the framework's own prop semantics. shut down when anything is still alive (SPEC.md, and never
   `window.confirm` per SPEC_impl's wry note); the archived session's view showing metadata and saying why there is no
   terminal; the archived filter toggle; and restart reachable from that view as the way back.

   Acceptance: archiving a live session confirms first, then tears down agent and tabs and keeps the session's metadata;
   the archived session leaves the default list and is reachable through the filter; opening it shows metadata and an
   explicit no-terminal reason; restart unarchives, creates a fresh terminal, and resumes the session's own conversation
   where capture exists; tabs do not return; after archive, restart can still read an existing attachment path;
   archiving twice is a no-op; a stale (unreachable-host) archive is refused with the host state named, like every other
   lifecycle operation.

6. **Provisioning.** SPEC.md and SPEC_impl already decide most of this; the plan's job is sequencing, the transparency
   flow, and the test strategy.

   **Discovery-first, per SPEC.md.** The remote transport probes by executing `farhelm internal stdio` over the user's
   ssh and completing a hello. The local transport runs the same probe directly, with no SSH-to-self. A supervisor that
   answers during an ADD is USED AS-IS and registered through the same first-contact path a manual add takes — never
   restarted, never replaced. Setup is proposed only after POSITIVELY ESTABLISHED absence: the transport succeeded, the
   command ran, and no supervisor answered. SSH or authentication failure, timeout, malformed hello, and protocol skew
   are concrete probe ERRORS under SPEC.md:410-419, never aliases for "not installed" and never provisioning offers. The
   local and remote variants consume the same concrete plan and run the same steps; transport is the difference, with
   process exec and file copy locally where the remote path uses ssh and sftp.

   **Payload source seam.** Provisioning reads install artifacts from an injectable payload source rather than reaching
   directly into release-only embedding. Item 6's shipped tests inject the locally built `target/debug/farhelm` as the
   supervisor payload; localhost is the same machine and architecture, so that is exactly the binary those isolated
   local and ssh-to-localhost cases install. Item 8 later supplies the release source from its embedded payload
   directory. This keeps dev helms payload-free without making item 6 untestable when it lands.

   **The step list.** Transfer the cross-compiled `farhelm` binary (plus a static tmux when the host has none or one
   below the 3.3 floor) into `~/.local/lib/farhelm/`; write the user-level systemd unit; `daemon-reload` and
   `enable --now`; `loginctl enable-linger` as the OPTIONAL step, proceeding without it and saying so where it needs
   privileges the user does not have (SPEC.md's rule); then dial and attach the supervisor to the already-registered
   host row. Each artifact is uploaded or copied to a temporary name in that FLAT directory and atomically renamed into
   place. The unit points at the flat binary path. There is no version directory and no `current` symlink: rename
   semantics already provide every property that machinery claimed. A running binary keeps its inode across replacement,
   a failed transfer leaves the installed file untouched, and sessions remain in tmux while a replaced supervisor
   restarts. SPEC_impl's provisioning section is updated in this same PR, under its standing sync rule, only to record
   the temporary-file-plus-rename install semantics. The HELM's unit is not installed here — provisioning installs
   supervisors; the helm's Linux unit ships with item 8.

   **Transparency and confirmation.** Before touching the host the helm states exactly what it will do in concrete terms
   — every path it will write, every unit it will create, what linger does, and the supervisor's startup behavior — and
   proceeds only on confirmation (SPEC.md's list, verbatim in intent). The plan value carries the boot promise
   CONDITIONALLY: the unit starts at boot if `loginctl enable-linger` succeeds, and otherwise starts at login, not at
   boot. The rendered confirmation says both outcomes before the optional step runs; a completed run where linger was
   refused reports the login-only degradation explicitly. The confirmation text is RENDERED FROM the same plan value the
   executor consumes, never written twice. That is the load-bearing part: a hand-maintained summary drifts from what the
   code does, and the promise being kept is that the helm says exactly what it is about to do.

   **Idempotent re-provision as recovery, and explicit update.** Re-running the ADD flow against a provisioned host
   short-circuits at discovery and re-registers, including from a brand-new helm whose registry was lost (SPEC.md). That
   short-circuit stays exactly as-is. An installed-but-stopped supervisor is converged instead: each step is written
   re-runnable — an identical payload transfer is skipped on a hash check, unit writes are content-compared, and
   `enable --now` is idempotent by systemd's own semantics.

   UPDATE is a separate, explicit user-triggered operation. It deliberately skips ADD's use-as-is short-circuit and runs
   the same converge plan against the existing install: an artifact whose hash differs is transferred to its temporary
   name and renamed into place, unit content is compared, and the supervisor is restarted. The sessions survive because
   tmux owns their processes and terminals and the unit's `KillMode=process` stops only the supervisor; the running old
   supervisor binary likewise remains a valid inode until the restart. The policy intentionally matches a manual
   supervisor run, where ending Farhelm detaches management without terminating the separately owned tmux sessions. This
   is the user-controlled update mechanism SPEC.md requires, not a background updater.

   Amended 2026-08-07, during this item's review: as written above, UPDATE "started the run" on its first request — five
   reviewers and SPEC.md's own transparency sentence agree that update deserves the SAME inspect-then-confirm handshake
   as ADD, and it ships that way: the first update request returns the concrete converge plan, a second request consumes
   its opaque one-use confirmation id and starts the run, and nothing mutates the host before confirmation. ADD's
   provision request likewise consumes its previously returned plan rather than accepting a bare destination. The review
   also hardened the shipped tests' isolation: probes run against nonce-scoped binary/state overrides only, unit files
   live in fixture directories exposed to the user manager through runtime links, and a checked guard verifies unit,
   file, and tmux teardown even when a test body fails.

   A failure leaves the host wherever it got to, reports which step failed with the host's own stderr (control-escaped,
   under the existing peer-text rule), and the remedy is running the operation again. No rollback: unwinding an install
   is more failure surface than it removes, and converge-on-rerun is the recovery SPEC.md already promises.

   **REST surface, and the shape long operations need.** `POST /api/hosts/probe` returns either the discovered
   supervisor or the concrete action plan; `POST /api/hosts/provision` starts an ADD run, and
   `POST /api/hosts/{id}/update` starts the explicit update run. Both return immediately with the run identity. Progress
   is NOT a second streaming endpoint: the run's state changes bump M6.75's feed and clients re-read it through
   `GET /api/hosts/{id}/provisioning`, which is exactly the feed's no-data contract and reuses the coalescing the helm
   already owns. In-flight authority lives in the helm — a second provision or update request for a host in flight is
   refused rather than queued — which is also what makes it correct across two browsers.

   The id exists before a provisioning run starts. Confirming remote setup first follows the registry's existing
   add-always-registers rule, creating the host row even for a machine with no reachable supervisor; the run then
   targets that registered id. The local host already has its guaranteed row. This ordering lets the progress read above
   keep its host-scoped URL without inventing a provisional identity.

   **Reach.** V1 targets Ubuntu only and only architectures with cross-compiled binaries (SPEC.md). The probe reads
   `/etc/os-release` and `uname -m`; anything outside that reports the manual path, which always remains available.
   Nothing here needs root.

   **Test isolation — the rules, restated because violating them was declared a parking offense.** Provisioning tests
   install binaries and create systemd user units; they MUST target throwaway unit names and isolated state and lib
   directories, never `~/.local/state/farhelm`, never `~/.local/lib/farhelm`, and never any unit name a real deployment
   would use. Transient-unit hygiene follows the cgroup tests' precedent: skip LOUDLY where no systemd user manager
   exists, with a visible reason, which is why `cargo test -- --show-output` is in the finish-work list. Shipped tests
   include both ssh-to-localhost/CI-shaped and local-no-SSH cases. The helm provisions `localhost` over the runner's own
   ssh in the first and executes/copies directly in the second, always into per-test temporary lib and state
   directories, with unit names carrying a per-test nonce, torn down by a guard that also runs on failure.
   `loginctl enable-linger` is never invoked against a real user by a shipped test; it is exercised as a planned ACTION
   (its presence in the plan value and in the confirmation text) and through a fake executor. Higher-fidelity
   verification against throwaway sudo-minted users happens on a NON-CI worker and is verification on top, never a
   substitute for the shipped tests and never a gate.

   Acceptance: a probe against a host running a supervisor registers it without proposing anything; positively
   established absence produces a concrete action plan and touches nothing; an SSH/auth failure surfaces its own error
   and never offers provisioning; confirming setup creates the host row before its run and progress reads use that id;
   provisioning localhost into isolated directories with the injected `target/debug/farhelm` payload yields a running
   supervisor under a nonce-named unit, a registered host, and operable sessions; the equivalent local case uses no SSH
   at all; re-running ADD is a no-op that re-registers; an explicit update converges an older install to a newer
   payload, restarts the supervisor, and leaves its running session uninterrupted in tmux; a failing step reports which
   step and the host's own message; the confirmation describes boot-start as conditional on linger, and a privilege
   refusal completes while reporting "starts at login, not at boot"; every test's units and directories are gone
   afterwards, including after a failure.

7. **UI provisioning.** The flows that make item 6 reachable.

   **Add-host.** Entering a destination probes first. Discovery finding a supervisor leaves the existing add path
   exactly as it is. Discovery finding nothing offers setup, shows the probe's concrete action list, and proceeds only
   on an explicit confirm. Confirmation first registers the destination under the existing add-always-registers rule,
   then submits provisioning against that host id; an unreachable supervisor does not mean an unaddressable host row.
   Any probe error renders that error and offers no setup; inability to reach a machine is not evidence that Farhelm is
   absent from it.

   **The local host.** M6 left `LOCAL_SUPERVISOR_NOT_RUNNING` rendering a `farhelm supervisor run` hint, with the
   manager's own docs recording that provisioning is M7's. The offer now replaces that hint as the PRIMARY affordance,
   with the manual command kept as secondary text — SPEC.md says the manual path always remains available, and a machine
   with no systemd user manager needs it. It invokes item 6's local transport, not SSH-to-self. Where no user manager
   exists, the offer is not shown at all and the hint is the whole remedy.

   **Long operations and `OpLock`.** Provisioning must not hold it. `OpLock` is a page-scoped token for operations that
   invalidate each other's premises within a click's timescale; holding it through a sixty-second install would freeze
   creates, host mutations, and the add form for the duration — a worse failure than the interleaving it prevents, and
   one that a second browser was never protected from anyway. The shape instead: `OpLock` wraps only the SUBMIT and is
   released as soon as the helm accepts the run; the row shows progress through the existing per-host busy marker plus
   item 6's feed-driven state read; and the actual mutual exclusion lives in the helm, where it is correct for every
   client at once.

   **Progress, update, and failure.** A per-host provisioning panel lists the steps with each one's outcome as it lands,
   the failing step's message verbatim, and a re-run action that is the same idempotent operation. A registered host
   also exposes the explicit UPDATE action: it shows the concrete converge plan, confirms before submitting, and then
   uses the same progress surface. It never turns into an automatic version check or background update.

   Honesty about what CI can prove here: Playwright cannot install to a host. These tests drive the UI against a helm
   whose provisioning executor is a test double, using the same injectable-seam discipline the connection cadences and
   the scope probe already use — the UI's contract is pinned without CI performing installs, and the real executor is
   item 6's Rust tests' business.

   Acceptance, as named Playwright tests: discovery-finds-nothing offers setup and shows the action list; nothing
   happens without confirmation; confirmation registers the remote host before submitting its id-scoped run; SSH failure
   shows the concrete error and no setup offer; the local host's guaranteed row receives its run without a new
   registration; the local offer replaces the manual hint (and does not when no user manager exists); progress advances
   without refresh; update shows and confirms its action list before submission; a failed step surfaces its message and
   the panel offers a re-run.

8. **Desktop bootstrap, release packaging, the absorbed M4 items, and the README.**

   **Desktop bootstrap comes first.** Packaging a thin client would produce an artifact that contradicts SPEC.md:46-50
   and SPEC_impl.md:702-704, while `crates/farhelm-ui/src/main.rs` records that the required embedded machinery is still
   deferred. Before the artifact work, the desktop target starts an in-process helm bound to loopback and serving the
   bundled UI, manages the bundled local supervisor through item 6's discovery-first local transport, and performs item
   3's token exchanges in-process. Plural is load-bearing: desktop REST uses native `reqwest`, while the feed and
   terminal WebSockets use the webview's JavaScript stack, and those are separate cookie jars. The app hands the token
   to BOTH stacks. `reqwest` exchanges it and explicitly attaches its resulting device cookie; the bootstrap delivers
   the token to the webview over IPC, never in a URL, and performs the same exchange inside the webview context so its
   WebSockets carry their own cookie. The app persists both device cookies in its own state and restores them on
   startup, like a browser would; a stack exchanges the token only when it has no cookie or the helm answers 401.
   Repeated app launches therefore reuse two device rows rather than minting two more every time. The helm keeps one
   enforcement path unchanged.

   The Mac lifecycle stays the one SPEC.md chose: no Mac system integration in v1, so the managed supervisor runs while
   the app runs; the transport and converge machinery are shared with the Linux helm, not duplicated as desktop glue.
   This phase lands first because the bundle should package the product, not presuppose code that no earlier item built.

   The bootstrap is cross-platform Rust. The normal Linux desktop build keeps
   `cargo check -p farhelm-ui --features desktop`, and an xvfb smoke harness starts the desktop target and proves its
   embedded helm serves the bundled UI, an authenticated native API call succeeds, an authenticated webview WebSocket
   connects, and the app reaches its managed local supervisor. That is useful automated coverage of both cookie jars and
   the shared machinery, not a claim about WKWebView or the Mac runtime; the Mac close-out remains the documented manual
   checklist.

   **Token management on Mac.** The bundle carries the CLI at `Farhelm.app/Contents/MacOS/farhelm`. A Mac user runs
   `Farhelm.app/Contents/MacOS/farhelm helm token show|rotate`; the README documents that exact path. The app uses the
   token internally for bootstrap, but v1 does not add an in-app show/rotate surface.

   **Supervisor artifacts.** `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, static, via cargo-zigbuild in
   CI — SPEC_impl's choice, already audited for rusqlite-bundled static musl; this PR builds it, it does not re-decide
   it.

   **Static tmux, and how tmux gets onto hosts that lack it.** Built in CI for both musl architectures from pinned
   upstream source tarballs (tmux plus libevent and ncurses) with recorded checksums, cached by version and architecture
   so it is not rebuilt every run. The workflow-dispatch macOS job builds the same pinned tmux for Apple arm64 from the
   same checksummed sources, caches it by version and architecture, and puts it in the app bundle — SPEC_impl.md:160-162
   requires a private tmux per platform, and macOS ships none. A cached tmux build adds minutes to that job, not tens of
   minutes; it does not materially change the 180-minute budget below. The alternatives are each ruled out by something
   already decided: downloading a third-party static build at provisioning time violates the no-third-party-downloads
   posture SPEC_impl states for exactly this payload; committing prebuilt binaries bloats the repo and obscures
   provenance; apt needs root, which SPEC.md forbids. The exact source pins are tmux 3.7b, libevent 2.1.13-stable, and
   ncurses 6.6, with every tarball checksum recorded in-repo beside the workflow. These are pins, not a moving "newest
   at build time" rule. The tmux SERIES choice remains ≥3.7 because below it the `bracket_paste_flag` capability is
   missing, which costs bracketed-paste restoration on reattach — a real feature loss SPEC_impl documents, and there is
   no reason to inherit it in a build we control. Ubuntu 24.04's 3.4 remains the compatibility floor CI exercises
   separately.

   **Payload embedding.** The musl binaries and target-appropriate private tmux builds ride inside the helm's own
   distribution (the Apple arm64 tmux in the Mac bundle; both Linux pairs in the Linux artifact), through a build script
   that reads a payload directory the release workflow populates. That embedded directory plugs into item 6's payload
   source seam; provisioning itself knows only the source interface. A DEV build embeds nothing and reports "this build
   carries no provisioning payloads" when its default embedded source is asked, while tests inject their local payload —
   otherwise every ordinary `cargo build` would require cross-compiled artifacts to be present, which would make the
   repo unbuildable for daily work. The tens-of-MB size cost is accepted, per SPEC_impl.

   **Unit files.** The supervisor unit template is what item 6 writes remotely or locally and points at the flat
   `~/.local/lib/farhelm/farhelm` path; the helm's user-level Linux unit ships in the release artifact for the user to
   install, since SPEC.md gives the helm the same layering and a reboot of the helm's machine must bring the web UI
   back.

   **Mac bundling.** A macOS CI job gated on `workflow_dispatch`, building `aarch64-apple-darwin` and `dx bundle`ing the
   .app with its managed supervisor and private tmux. The completed .app enters the same release-publication path as the
   Linux artifacts and is attached to the release as a downloadable asset; a temporary workflow artifact is not the
   product surface. Dispatch gating remains because macOS runner minutes are the scarcest CI resource and nothing in the
   normal PR loop needs a bundle. CI caps each macOS attempt at 60 minutes; the dispatching operator records cumulative
   runtime and stops retrying once the release candidate reaches 180 minutes. GitHub Actions has no repository-local
   counter spanning repeated dispatches, so this total is an operator-enforced release constraint, not something the
   workflow can truthfully claim to police. A timed-out attempt still fails — failure is the signal to fix the build,
   not permission to silently skip the app. Codesigning and notarization are out (see Out).

   **Absorbed M4 item (a): the pasted-image-name heuristic.** The observed failure is an image FILE copied in Finder and
   pasted on macOS/WKWebView publishing as a generated `pasted-<n>.png` instead of its own name — the documented cost of
   `classify`'s origin heuristic. What is designable from Linux, and is what this PR ships: make `classify` prefer a
   `DataTransferItem`'s `File.name` whenever one is present and is not an engine-synthetic placeholder, rather than
   deciding by origin; make the name decision a PURE function over captured clipboard facts, unit-tested under M6.5's
   `node --test` harness, so that the Mac's real facts later become a FIXTURE rather than a code change; and add a debug
   affordance that dumps the clipboard facts (item count, kinds, types, `File` names, `lastModified`, item order) into
   the UI so the manual round captures them by copy-paste instead of console archaeology. The WKWebView capture itself
   is a documented MANUAL checklist for the user — there is no WebDriver for WKWebView on macOS, which SPEC_impl already
   names as the acknowledged gap — and NO wry-native clipboard hook is built in v1 unless those facts prove the DOM path
   cannot carry the name. Building a native hook before the facts exist would be guessing at which engine behavior is
   the problem.

   **Absorbed M4 item (b): remote-paste latency.** Likewise a manual checklist, because it needs a Mac and a real link:
   paste a screenshot into a remote session and measure from paste to path-at-cursor, along PLAN_M4.md acceptance 9's
   `--ssh`-shaped path. The number lands in this plan's ledger or a lore note when it exists. Not a gate, and explicitly
   still unmeasured until then.

   **The README refresh** closes the milestone, as it has closed every milestone: the "Trying it" section moves from
   M6.75's manual stack to install-and-provision, and the token bootstrap is documented where a first-time user meets
   it.

   Acceptance: the xvfb smoke harness proves the desktop target starts its embedded helm, serves the bundled UI, makes
   an authenticated native API call, connects an authenticated webview WebSocket, reaches its managed local supervisor,
   and reuses both persisted device cookies across an app restart without adding device rows; the release workflow
   produces both musl supervisor artifacts and the Linux tmux builds from the exact pinned, checksummed sources; a
   release-built helm carries its payloads through item 6's source seam and a dev-built one says it does not; the macOS
   job runs only on `workflow_dispatch`, builds the cached pinned Apple arm64 tmux, produces a .app carrying the
   embedded helm, supervisor, tmux, and documented CLI path, publishes that .app as a release asset alongside the Linux
   artifacts, fails an attempt at 60 minutes, and is stopped by the operator at 180 cumulative minutes; the
   clipboard-name decision is a pure function with `node --test` coverage and a facts-dump affordance exists; the manual
   checklists are written down; the README describes the shipped install and Mac token-management paths.

### Out (deliberately)

Notifications (non-goal). Automatic initial-prompt delivery (non-goal; SPEC.md records the shape it would take). Profile
syncing across hosts (post-v1). TLS and non-loopback binding (post-v1; the token is not transport security and this plan
does not pretend otherwise). Multi-user or multi-writer anything. Spawning onto a host other than the session's own
(SPEC.md: v1 is own-host only). An unarchive VERB (restart is the only path SPEC.md names). Supervisor-edge wire push —
still declined, see the review questions. Automatic or background updates: SPEC.md requires user-controlled updates, so
item 6's explicit "update this host" operation is the only update path. Codesigning and notarization of the Mac app,
which need an Apple Developer account and CI secrets this plan does not assume; the release asset is a locally-runnable
.app. An in-app token show/rotate affordance (post-v1; SPEC.md:430-433's disjunction is satisfied by the bundled CLI).
Non-Ubuntu provisioning targets, which fall back to the manual path. A wry-native clipboard hook (item 8(a)'s
conditional). Any change to the flake ledger or the seam-requiring test debts PLAN.md parks — both stay exactly where
they are.

## Testing decisions (settled while planning)

Auth splits by layer: Rust tests over the store, router, and middleware for recoverable token storage, enforcement (401
on REST and on WS-before-upgrade), the two unauthenticated routes, the stamp surviving a 401, and rotation deleting
device sessions in one transaction. Playwright owns the human-facing half: the prompt, persistent once-per-device
exchange across a browser restart, rotation logging an open client out, and rotation dropping both an already-open feed
and an already-open terminal socket. The e2e harness's authenticated `storageState` is set up once so no later spec pays
for auth.

Spawn is pinned at four levels. Supervisor integration tests cover admission and authority (bad credential, an ordinary
local peer with absent auth retaining full authority, non-create message from a session-authenticated peer, forged
parent), schema-upgrade credential minting, profile-name resolution inside create, idempotent replay, fingerprint
conflict including a changed parent, and tombstone expiry with the child. CLI tests pin stdout, stderr, exit status, cwd
canonicalization, required environment, and the pre-upgrade restart remedy. Helm and browser tests cover last-used
profile convergence, the parent query/predicate/control, and a fake agent spawning a child that appears without refresh.
Finally, an env-gated `FARHELM_REAL_AGENT=1` Playwright leg reuses M6.5's helper for SPEC acceptance test 7 and skips
loudly otherwise. The fake agent grows a `spawn` script, and SPEC_impl's script list is updated in that same PR under
its standing sync rule.

Archive is pinned in the supervisor (teardown reaches agent and tabs, metadata and attachments survive, restart
unarchives with a fresh terminal and a resumed conversation that can still read an attachment path, tabs do not return,
double-archive is a no-op), in the helm (the include-archived toggle's default-off behavior, the recorded mutation, the
feed bump), and in Playwright (the confirmation naming what dies, the archived view's no-terminal reason, the toggle,
restart-from-archived).

Provisioning's shipped tests obey the isolation rules in item 6 without exception, inject the local debug binary through
the payload source, and cover probe-error taxonomy, ssh-to-localhost, direct local transport with no SSH, registration
before id-scoped submission, conditional linger wording, and explicit converge-to-newer with a session surviving the
supervisor restart. The UI's provisioning tests run against an injected fake executor; real-host fidelity is a non-CI
worker's manual job and is never a gate. The desktop bootstrap's cross-platform machinery has the Linux build and xvfb
smoke harness, including authenticated native API and webview WebSocket legs plus persisted-cookie reuse across restart.
The macOS bundling job is likewise not a PR gate — it proves the app BUILDS and that a release carries it, never that it
works on Mac; WKWebView and Mac runtime behavior remain SPEC_impl's acknowledged manual-test gap.

## Acceptance

M7's PR stack is complete when the automated gates below pass and the Mac checklist below EXISTS. Performing that
checklist is the user's v1 close-out under the standing decision that Mac verification is manual; none of its entries is
silently promoted to CI or silently dropped.

The always-on stack automates the parts of SPEC.md:473-486 that can be proved without a Mac or vendor credentials, and
keeps the real-agent remainder as a named env-gated leg:

1. The browser UI is token-authenticated, remembers its persistent device session across a browser restart, and loses
   every device session and live socket on rotation.
2. Provisioning succeeds through both ssh-to-localhost and direct local-no-SSH transports in isolated directories,
   registers an operable supervisor, and updates it to a newer payload without interrupting its tmux-held session. This
   is localhost-shaped coverage of SPEC acceptance item 1, not a claim about a fresh real Ubuntu host reached from a
   Mac.
3. A fake agent invokes `farhelm spawn`, and its child appears in an open browser without refresh. The
   `FARHELM_REAL_AGENT=1` leg carries SPEC acceptance item 7's full automated contract — real Claude creates a workspace
   with `jj workspace`, invokes spawn, and the child appears without refresh — and skips loudly when deliberately
   ungated.
4. Every item-level acceptance test and the repository's full CI gate pass.

The user close-out checklist maps the native-app aspects of SPEC acceptance items 1 through 7 directly:

1. From the native Mac app, provision a fresh Ubuntu host using only passwordless SSH; verify the supervisor starts
   without root, registers, and runs sessions, then use `Farhelm.app/Contents/MacOS/farhelm helm token show` and open
   the same helm's token-authenticated web UI.
2. Create an official Claude Code session in one action in an existing `jj` workspace where Git reports detached HEAD.
3. Create a local Mac session the same way and verify it and the remote session appear in one list.
4. Paste a Mac screenshot into the remote terminal; verify the path appears at the cursor and Claude reads the file,
   while capturing the WKWebView clipboard facts and remote-paste latency from item 8's two manual checklists.
5. Quit and relaunch the app and verify both sessions and terminal state remain; reboot the Mac and verify the remote
   session is untouched while the local session is interrupted and offers conversation resume.
6. Attach to the remote session from the web UI and verify the native app visibly detaches.
7. Ask real Claude to create a new `jj workspace` and invoke the spawn CLI; verify the child appears without refresh.

## Order of work

Each step leaves something runnable; later steps only add. Tests ride with their step.

1. This plan, plus the PLAN.md header update it implies (M6.75 moves to history).
2. Proto v11 — the complete M7 vocabulary, one bump.
3. Web-token auth and device sessions (opening with the `api_router`/`build_router` split), plus the e2e-harness auth
   migration.
4. Supervisor spawn and the `farhelm spawn` CLI, with the parent-reference filter M6.75 deferred to it.
5. Archive across supervisor, helm, and UI (opening with the RowActions/RowState grouping).
6. Provisioning in the helm.
7. UI provisioning flows.
8. Desktop bootstrap, release packaging, the two absorbed M4 items, and the closing README refresh.

Steps 4 and 5 are the one pair whose order was genuinely open. They stay in this order because the parent-reference
filter M6.75 assigned to "M7, beside the feature that mints parent references" belongs to step 4, and letting the row
and filter surfaces grow the parent filter and then the include-archived toggle means touching them once each rather
than reopening step 5's work. Steps 6 and 7 may run as parallel tracks once step 6 freezes the probe and provision
contracts, per the goal's standing recipe.

## Review questions for this plan's PR

These are open, not rhetorical, and the first two change what gets built.

1. **Reservation tombstone scope for INTERACTIVE creates.** This plan keeps them permanent (M3's decision) and bounds
   only spawn's. The counter-case is real: the store's own docs record `create_reservations` as the one table that grows
   without bound, and making every scope session-lifetime would finally close that, at the cost of a long-deleted
   session's create key becoming reusable. Keeping permanent leaves the digest-or-expiry work the store's docs describe
   unowned.

2. **The ~3s supervisor-drain bound versus SPEC.md acceptance test 7.** M6.75 declined a supervisor-edge push and said
   the evidence for one would be M7-era. This plan reads "the child appears without refresh" as a statement about user
   action rather than latency and declines again. If a reviewer reads it as requiring something tighter, the
   supervisor-edge push has to ride item 2's bump — which makes this a question that must be answered at THIS PR, not
   discovered during item 4.

3. **The include-archived toggle and the count banner.** M6.75's banner reads "N matching of M sessions", where M is the
   overall fleet total. With `include_archived` off, the default view hides rows that M still counts. This plan keeps M
   meaning the fleet total on the grounds that archived sessions are part of the fleet; the alternative reading is that
   M should exclude them so the default view's two numbers agree.

## Debts and parked

- Every seam-requiring test debt PLAN.md's M6.5 ledger holds stays parked, unchanged. So does the flake ledger,
  including the two undecided CI-quieting candidates and the standing rule that a third supervisor SIGSEGV is chased
  rather than rerun.
- WKWebView clipboard facts (File name, `lastModified`, item order) remain a manual checklist. There is no WebDriver for
  WKWebView on macOS, so this cannot become automated coverage; item 8 makes the capture cheap and makes the resulting
  facts a fixture.
- Remote-paste latency remains unmeasured until the manual round runs.
- Provisioning against real Ubuntu hosts and real sudo-minted throwaway users is non-CI worker verification, not a gate.
  A regression there is caught by a human running the checklist.
- WKWebView and Mac-specific runtime behavior have no automated coverage and will not get any; the Linux xvfb harness
  covers only the shared desktop bootstrap machinery. Codesigning and notarization are deferred with the Mac checks.
- `HostRow`'s eighteen props stay ungrouped, deliberately (assessment U7 applies where props are growing).
- If review question 1 keeps interactive creates permanent, the digest-or-expiry work bounding `create_reservations`
  stays unowned and belongs on this ledger for whoever picks it up.

## Risks retired by this milestone

- The helm stops being an unauthenticated API on loopback, which is the last place a "it's only local" assumption was
  load-bearing — and it happens BEFORE three more surfaces are built on top of it, rather than as a retrofit across all
  of them.
- Provisioning turns the install story from a paragraph in a README into a tested path, which is also what makes
  SPEC.md's acceptance test 1 something a machine can attempt.
- Spawn puts the in-session edge under a credential from the first line of code, rather than shipping an open unix
  socket and tightening it later.
- Archive gives the durable session list a way to shrink, which is the thing a year of dogfooding otherwise makes
  unusable.
- The embedded-helm Mac bundle and the musl payloads make the version-skew story concrete: a provisioned host runs
  exactly what its provisioning helm carried, so mixed-version fleets stay the deliberate case rather than the
  accidental one.
