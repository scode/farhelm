# Session-list ingress admits ids the create path refuses

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

None today; the fix keeps a future helm route from letting a compromised remote machine redirect a Stop or Close click
to a different helm action.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F30 / SEC-LIST-INGRESS-ID-VALIDATION`, tagged **possible**. Anchors and title: `manager.rs:750-790`, `client.rs:369`,
`crates/farhelm-ui/src/api.rs:843-860` — Session-list ingress admits empty, control-character and dot-segment ids that
create refuses

Session ids reach the helm from supervisors through two main ingress points, and the two validate differently:

- `created_session` (`client.rs:369`), for create replies, refuses empty ids, ids over the size cap, and ids containing
  control characters.
- `drain_sessions` (`manager.rs:750-790`), for every list refresh, checks only the size cap and duplicates. Empty ids,
  control characters, and ids such as `.` or `..` are all accepted into the cache and the UI.

The UI's defence against a supervisor-chosen id redirecting a request is `encode_path_segment`
(`crates/farhelm-ui/src/api.rs:843-860`). Its doc claims that percent-encoding `.` as `%2E` stops URL dot-segment
resolution. Under the WHATWG URL Standard, which browsers and webviews follow, it does not: `%2e` and `%2E` count as
dots when resolving path segments. The reviewer checked with a throwaway `node -e`:

- `/api/sessions/%2E%2E/stop` resolves to `/api/stop`;
- `/api/sessions/%2E/stop` resolves to `/api/sessions/stop`;
- `/api/sessions/S/tabs/%2E%2E` resolves to `/api/sessions/S/`.

With today's router, none of these reaches a real route with a matching method (they get 404 or 405). So this is a
broken defensive layer, not a current exploit. A future `/api/<verb>` route would make it exploitable.

Suggested fix: use one shared session-id validator at every peer ingress that refuses empty ids, control characters and
`.`/`..`. Ideally restrict ids to a conservative character set, since supervisors mint UUIDs. Separately, fix the UI
encoder's doc and behaviour, for example by refusing dot-only segments outright.

User-visible consequence: none today. The fix stops a future helm route from letting a compromised remote machine
redirect a Stop or Close click to a different helm action.
