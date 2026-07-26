// Playwright drives the WEB build in headless Chromium against a real
// helm, supervisor, private tmux, and fake agent — no mocks anywhere in
// the stack (PLAN_M1.md: this is the canonical GUI verification path for
// agents; the desktop webview is validated manually).
//
// The stack is booted by start-stack.sh; `cargo build` and
// `dx build --platform web --release` must have run first (CI does; locally see
// that script's header).
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  // The stack is one shared helm with ONE session; tests interfere with
  // each other by design (takeover semantics), so run serially.
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  use: {
    baseURL: "http://127.0.0.1:7434",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "bash ./start-stack.sh",
    url: "http://127.0.0.1:7434/api/sessions",
    reuseExistingServer: false,
    stdout: "pipe",
    stderr: "pipe",
    timeout: 30_000,
    // Without this Playwright SIGKILLs the process group, which no trap
    // can catch — and the private tmux server (which daemonizes out of
    // the group) would survive every run. SIGTERM lets start-stack.sh
    // reap its own supervisor and tmux server.
    gracefulShutdown: { signal: "SIGTERM", timeout: 5_000 },
  },
});
