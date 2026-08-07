// Playwright drives the WEB build against a real helm, supervisor,
// private tmux, and fake agent — no mocks anywhere in the stack
// (PLAN_M1.md: this is the canonical GUI verification path for agents).
//
// Two engines run: Chromium, and WebKit as a stand-in for the desktop
// app's actual renderer family (WKWebView on macOS, WebKitGTK on Linux).
// This covers the WEB build under that engine — rendering, input, JS
// API behavior — not the desktop shell: wry-layer integration bugs
// (like the eval-channel death manual testing found on macOS) live
// outside anything a browser run can reach, and stay with the desktop
// smoke harness and manual passes. Firefox is not included: nothing in
// the desktop app embeds Gecko, so it would add runtime without
// covering a real target.
import { defineConfig, devices } from "@playwright/test";
import {
  AUTH_STORAGE_STATE_PATH,
  harnessAuthorizationHeaders,
} from "./tests/helpers/device-auth";

const authorizationHeaders = harnessAuthorizationHeaders();

export default defineConfig({
  testDir: "./tests",
  // The stack is one shared helm with ONE session; tests interfere with
  // each other by design (takeover semantics), so run serially. This
  // also serializes ACROSS projects: `workers` caps the whole run, not
  // each project, and both projects point at the one webServer instance
  // below, so a second worker running WebKit tests concurrently with
  // Chromium ones would race on that shared session server-side. Running
  // two engines therefore doubles wall-clock time rather than cost —
  // there is no safe way to claw that back without splitting the shared
  // session per project.
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  use: {
    baseURL: "http://127.0.0.1:7434",
    // These two options are one credential in two browser transports.
    // Playwright injects `use` options into the request fixture and into
    // manual browser.newContext() calls as well as its default page context,
    // so secondary clients cannot silently fall back to unauthenticated I/O.
    storageState: AUTH_STORAGE_STATE_PATH,
    extraHTTPHeaders: authorizationHeaders,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  globalSetup: "./global-setup.ts",
  // The per-engine stack reset lives in terminal.spec.ts's own
  // `beforeAll`, not in a setup project or a name-ordered spec file:
  // Playwright does not guarantee alphabetical file ordering, and
  // project `dependencies` cannot express "webkit's reset runs AFTER
  // chromium's tests" without dragging the whole chromium suite into a
  // `--project=webkit` run. A `beforeAll` runs at each project's entry
  // into the file, in order by construction, and its failure fails the
  // file's tests rather than letting them run against half-reset state.
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  webServer: {
    command: "bash ./start-stack.sh",
    // The API correctly answers 401 before global setup has exchanged the
    // token. The public static bundle is therefore the readiness probe.
    url: "http://127.0.0.1:7434/",
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
