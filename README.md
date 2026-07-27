# farhelm

Run coding agents (Claude Code, Codex, other terminal agents) on machines you control and supervise all of them from one
interface, through their real TUIs. See SPEC.md for what this is and is not, SPEC_impl.md for how it is built and why,
and PLAN.md for where the build currently stands.

NOTE: This is milestone-1 software: one session, one host, argv-driven setup. Usable for real work, minimal in
everything else. Two caveats worth knowing before that real work: the helm's loopback API carries no authentication yet
(the web token is a later milestone), so any local account on the helm's machine can drive your sessions — treat
multi-user hosts accordingly; and the `--agent` invocation is ordinary argv, visible to every local user via `ps`, so
credentials do not belong in it.

## Trying it (M1)

Prerequisites: Rust with the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`), tmux on every
host involved, and `cargo binstall dioxus-cli@0.7.9` (or `cargo install dioxus-cli@0.7.9` — match the workspace's dioxus
version) for the web UI build.

NOTE on tmux versions: 3.3 or newer runs everything. Restoring bracketed paste when you reattach additionally needs 3.7,
because that is when tmux gained the `bracket_paste_flag` format — below it the supervisor logs a warning (once, the
first time a session is attached) and everything else still works. Ubuntu 24.04 ships 3.4.

- Build: `cargo build`, then `(cd crates/farhelm-ui && dx build --platform web --release)`.
- On the host that will run the agent (or this machine): `target/debug/farhelm supervisor run`.
- Locally, against a supervisor on this machine:
  `target/debug/farhelm helm run --ui-dist target/dx/farhelm-ui/release/web/public --cwd ~/some/project --agent claude`
- Against a remote host (passwordless ssh assumed; copy the binary there first): add
  `--ssh user@host --remote-farhelm /path/to/farhelm` to the helm command. Use an absolute `--cwd` there — it names a
  directory on the target host, and your local shell would expand a `~` against the wrong home.
- Open the printed loopback URL in a browser: a session list (title, working directory, invocation, and a status —
  alive, or exited with the code when known), refreshing on its own every few seconds. Click a row to open its terminal;
  a back control returns to the list. Close the tab, reopen it later: same session, scrollback intact, the agent never
  noticed.

The desktop window is the same UI in a wry webview: `cargo run -p farhelm-ui --features desktop` with `FARHELM_URL`
pointing at the helm (default `http://127.0.0.1:7433`).

## Development

`AGENTS.md` has the conventions and the finish-work checks. End-to-end tests: `cargo test` (Rust, including real-tmux
integration), and `cd e2e && npx playwright test` (browser against a real stack — needs `npm install` and
`npx playwright install chromium` once). `lore/` holds historical decision records; read `lore/AGENTS.md` before
touching it.
