# The sensitive-host YOLO guard misses Cursor's short `-f` flag

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

A host that asks before YOLO launches (every host, by default) should stop and ask before starting an agent that skips
approval prompts. Cursor's documented short flag, `agent -f` (short for `--force`, which `--yolo` aliases), starts with
no confirmation, and the sidebar does not badge the session as YOLO either. The agent then runs with approvals off on
that host.

## Details

Source: whole-codebase review, 2026-09-30. Two slices (helm store, CLI/protocol) found overlapping parts of the YOLO
guard's recognition gaps independently.

Rebase note: when this was rebased onto main at 1ec60cc, two items from a separate review already covered the other
spellings this item originally listed, so it was narrowed to what they do not cover, and its extra material was merged
into them: the `env` wrapper is `yolo-guard-misses-env-prefix.md`, and Codex's `-a never -s danger-full-access` option
form is `yolo-guard-misses-codex-option-form.md`. The three share one classifier and are best fixed together.

SPEC.md (Session creation, YOLO paragraph) says a launch is YOLO when "its command line carries a flag its vendor
documents as skipping approval prompts", and that the helm enforces the confirmation on every create, clone, replace,
replace-with and restart-with, "so no client can skip it".

Raw and profile invocations are classified by one chain: `crates/farhelm-helm/src/yolo_guard.rs:43-58` (`create_is_yolo`
→ `invocation_is_yolo` → `shell_words::split` → `farhelm_proto::yolo::argv_is_yolo`). Reach: REST raw `invocation`,
user-edited catalog profiles (`profiles.rs` only runs `validate_profile_fields`), `mode_from_source` for clone/replace
of a raw or profile-backed source, agent `create --invocation`/`--profile`, and `ResolveProfile` for `farhelm spawn`
(`agent_requests.rs:353`). Structured composer launches are unaffected, because `selection_is_yolo` classifies them.

In `crates/farhelm-proto/src/yolo.rs`, `INVOCATION_MARKERS` (~98-113) lists only `--force` and `--yolo` for `agent` and
`cursor-agent`, and `argv_is_yolo` (~175) matches flags by exact string equality, so `agent -f` is not YOLO. Cursor's
CLI reference (https://cursor.com/docs/cli/reference/parameters) lists "`-f, --force` Force allow commands unless
explicitly denied" and "`--yolo` Alias for `--force`". `invocation_marker`, which drives the sidebar badge, reads the
same table and misses it too.

Consequence: `yolo_guard::check` receives `is_yolo == false` and returns `Ok`; the create is dispatched and the agent
starts with approvals disabled, with no confirmation and no badge.

Verify: add `"agent -f"` and `"cursor-agent -f"` to the `yolo` list in
`raw_command_lines_are_classified_by_program_and_leading_flag` (`yolo.rs`). Both fail today.

Fix: add `-f` to the `agent` and `cursor-agent` entries, and audit other vendors' documented short aliases while there.

### Extension at de774a1ee8815ce833da77deac593a55d82f7be3: an explicit Pi agent kind is ignored

Source: gap-filling review pass, 2026-09-30, slice helm-store2. The reviewer proposed dropping this as covered by the
wrapper-limit sentence now in `yolo-guard-misses-env-prefix.md`; the independent drop checker found it is not an
arbitrary wrapper and asked for it to be added here instead.

SPEC.md:442-445 makes every Pi launch a YOLO launch ("its structured permission is YOLO, which makes every Pi launch one
... The helm enforces this, so no client can skip it"). A raw create body can carry an explicit agent kind
(`crates/farhelm-helm/src/sessions.rs:924`, accepted with a raw invocation at :2344), and so can a user profile, which
is validated only for size and NUL bytes (`crates/farhelm-proto/src/lib.rs:1255`). The supervisor execs any argv
regardless of kind. Yet `create_is_yolo` (`crates/farhelm-helm/src/yolo_guard.rs:43-51`, the Raw and ResolvedProfile
arms at :45 and :50) classifies those launches only through `argv_is_yolo`, which recognizes Pi only by the basename
`pi` (`crates/farhelm-proto/src/yolo.rs:136,179`).

Trigger: a raw create with agent kind Pi and a program not named `pi` (for example `node …/pi/cli.js`), or a Pi profile
with such an invocation, including clone and replace of such a session. Consequence: Pi, which has no approval gate,
starts on a sensitive host without the confirmation and without the YOLO badge. The wrapper-limit resolution suggested
in `yolo-guard-misses-env-prefix.md` does not fit, because here the helm is told outright that the launch is Pi.

Fix: treat an effective agent kind of Pi as YOLO in `create_is_yolo` and in the clone, replace and restart-with paths.

Not the same as the `env A=b pi` case in `yolo-guard-misses-env-prefix.md`: that one still reaches a program named `pi`;
here the program has another name and only the declared agent kind says it is Pi.
