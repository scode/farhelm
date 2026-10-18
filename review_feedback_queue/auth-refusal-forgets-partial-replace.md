# Signing in again can duplicate a partial replacement

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

If Replace starts the new session but cannot remove the original, a later authentication refusal can forget which
replacement it started. Retrying after browser sign-in can start another replacement agent in the same folder.

## Details

Suggested bucket: `high`, for duplicate agents. Confidence: confirmed static classification, caller/server path and
within-task scheduling; the complete browser sequence was not reproduced. Provenance: retained `ui-http-and-auth-1/f1`,
corroborated by `ui-app-state-1/f2`, including their second-turn synthesis. Proposed drop: none. Concurrent-agent loss
of work was considered but not established; this finding does not claim observed file loss.

Replace with key K can create replacement R and then retain source S when its delete precondition refuses
(`crates/farhelm-helm/src/sessions.rs:3362–3409,3508–3554`). Retrying K can be the request that first discovers missing
or revoked browser credentials. The auth middleware refuses that retry before its handler
(`crates/farhelm-helm/src/auth.rs:360–397`), so its 401 proves only that this request was not admitted. It says nothing
about the earlier accepted create under K.

`crates/farhelm-ui/src/api.rs:2253–2279` nevertheless treats every `Unauthenticated` or `Refused` send error as answered
and discards the Replace key. This includes a recognized authentication 401, an unrecognized 401 and a failure reading
the 401 body; only a `Request` error retains it. The header and sidebar clear the shared intent immediately after the
API returns (`session_view.rs:1650–1654`; `list/view.rs:2394–2397`). `require_token` only writes a signal
(`auth.rs:429–432`), and no await intervenes before that clear. Pinned Dioxus 0.7.10 schedules the resulting render
rather than synchronously unmounting the caller. `AppBody` owns the key above the authentication branch
(`lib.rs:1614,1694–1703`), so the owner survives sign-in with an empty slot. Retrying S then mints K2 and creates R2.

Another request can trigger sign-in and cancel this caller first, preserving K; that competing ordering does not prevent
Replace consuming its own 401 first. The events feed reports revision/down rather than opening sign-in itself
(`assets/events.js:204–220,318–340`). Cleared browser storage while an established feed stays connected is one trigger;
credential rotation or eviction depends on which REST response arrives first. Ordinary native credentials are not
revoked. An initial refusal for a never-accepted K is safe to retire; an unresolved K previously accepted is not.

Preserve unresolved Replace identity on authentication refusal, and retire it only on successful completion or durable
evidence that nothing was created. A focused regression should commit the replacement, refuse source deletion, make the
retry receive middleware 401, sign in, and verify that retry creates no second replacement. New, Clone and Replace with
have different refusal classification and component-local form keys; this finding concerns plain Replace's explicit
clearing of a key whose owner survives the sign-in branch.

Possible cover: SPEC.md, “Signing in again,” allows losing browser forms, drafts and pending reports and discourages
complex preservation. `TRIAGE_OUTCOMES.md`, “browser-signin-loses-action-outcomes.md,” accepts lost reports while the
server result remains. Neither explicitly accepts duplicate creates through a surviving Replace control. SPEC.md:782–790
instead retains partial-result identity. The prior ledger entries “plain-replace-loses-retry-identity.md,”
“replace-retry-key-splits-across-controls.md” and “replace-retries-replay-settled-create-failure.md” concern adjacent
fixes with different triggers. The queued `launcher-retries-ignore-settled-refusal-proof.md` concerns retaining a key
after a settled refusal in other launchers, the opposite error. No exact Planned, BUGS or filter coverage was found.
