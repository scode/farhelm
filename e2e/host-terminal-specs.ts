// The main browser suite's run against terminals in terminal hosts: the
// switch that turns it on, and the spec files it runs.
//
// The suite was written against old terminals (tmux, drawn by xterm.js), and
// its ordinary runs stay exactly that. Parity of the new terminal (a terminal
// host, drawn by the grid component) is shown by running the same specs
// against stacks whose local supervisor creates every terminal in a host:
// `FARHELM_E2E_SUITE_TERMINALS=host npx playwright test` (see
// `.agents/validation.md`). The terminal helpers in tests/helpers/ dispatch on
// the kind of terminal they find, so a product-level assertion is written
// once for both kinds.
//
// The list holds every spec file that drives a terminal, directly or through
// the terminal-family helpers (tests/helpers/term.ts, terminal-readiness.ts,
// terminal-suite.ts, real-agent.ts): their tests hold on both kinds. A test
// about the old terminal alone skips itself with `onHostTerminals()`, saying
// why and where the grid's counterpart is
// pinned. Spec files that never reach a terminal are left out: a host run has
// nothing to add to them. Terminal files left out whole, because every test in
// them is about xterm.js itself, are `HOST_TERMINAL_LEFT_OUT`, with the reason.
// The config refuses a host run when a listed or left-out file does not
// exist, when a file is in both, or when a spec that imports one of those
// helpers is in neither, so the account stays complete.

/**
 * The environment variable naming the kind of terminal the main suite's
 * stacks create: unset for tmux (start-stack.sh pins the stacks' local
 * supervisors to it, since a bare supervisor creates terminal hosts), `host`
 * for terminal hosts. Read by playwright.config.ts and, through the worker
 * processes' inherited environment, by `onHostTerminals()` in
 * tests/helpers/term.ts. Nothing sets it from inside a test.
 */
export const SUITE_TERMINALS_ENV = "FARHELM_E2E_SUITE_TERMINALS";

/**
 * The main config's metadata key naming its run's terminal kind (`host` on
 * the host-terminal run, `default` otherwise). Global setup and the per-test
 * premise check (tests/helpers/host-premise.ts) read this rather than the
 * variable, which other configs' runners may inherit; it also shows the
 * run's kind in a report.
 */
export const SUITE_TERMINALS_METADATA = "suiteTerminals";

/** The spec files (relative to tests/) a host-terminal run runs. */
export const HOST_TERMINAL_SPECS: ReadonlySet<string> = new Set([
  "agent-relay.spec.ts",
  "appearance.spec.ts",
  "approval-layout.spec.ts",
  "auth.spec.ts",
  "buttons.spec.ts",
  "chrome.spec.ts",
  "claude-trust.spec.ts",
  "clone.spec.ts",
  "composer-word-search.spec.ts",
  "custom-permissions.spec.ts",
  "destination-authority.spec.ts",
  "github-checkout-composer.spec.ts",
  "github-checkouts.spec.ts",
  "guarded-permissions.spec.ts",
  "header.spec.ts",
  "host-picker.spec.ts",
  "host-short-names.spec.ts",
  "hosts-panel.spec.ts",
  "interface-fonts.spec.ts",
  "keyboard-scroll.spec.ts",
  "keyboard-shortcuts.spec.ts",
  "launch-refusal.spec.ts",
  "launch-progress.spec.ts",
  "launcher-tabs.spec.ts",
  "listing-mutation-fence.spec.ts",
  "menu-highlight.spec.ts",
  "modal-focus.spec.ts",
  "modal-layout.spec.ts",
  "modal-trap.spec.ts",
  "mouse-modes.spec.ts",
  "named-conversations.spec.ts",
  "notifications.spec.ts",
  "popover-expanded-host-movement.spec.ts",
  "popover-header-movement.spec.ts",
  "popover-host-movement.spec.ts",
  "popover-identity-movement.spec.ts",
  "popover-movement.spec.ts",
  "popover-width-movement.spec.ts",
  "quick-launch.spec.ts",
  "quick-switcher.spec.ts",
  "real-agent.spec.ts",
  "remembered-permissions.spec.ts",
  "replace.spec.ts",
  "restart-with.spec.ts",
  "restart-replace-progress.spec.ts",
  "session-action-shortcuts.spec.ts",
  "session-confirmations.spec.ts",
  "session-view-claims.spec.ts",
  "settings.spec.ts",
  "sidebar-resize.spec.ts",
  "sidebar.spec.ts",
  "spawn.spec.ts",
  "status-filter.spec.ts",
  "template-list-shortcut.spec.ts",
  "terminal-attachments.spec.ts",
  "terminal-clipboard.spec.ts",
  "terminal-create-idempotency.spec.ts",
  "terminal-files.spec.ts",
  "terminal-flood.spec.ts",
  "terminal-font.spec.ts",
  "terminal-keys.spec.ts",
  "terminal-links.spec.ts",
  "terminal-migration.spec.ts",
  "terminal-multihost.spec.ts",
  "terminal-reconnect.spec.ts",
  "terminal-replay-rename.spec.ts",
  "terminal-restart.spec.ts",
  "terminal-tabs.spec.ts",
  "terminal-unicode.spec.ts",
  "terminal.spec.ts",
  "tooltip-coverage.spec.ts",
  "tooltip.spec.ts",
  "trash-dialog.spec.ts",
  "ui-plan-regressions.spec.ts",
  "welcome.spec.ts",
  "window-chrome.spec.ts",
  "workspace-trust.spec.ts",
]);

/**
 * Terminal spec files the host run leaves out whole, with why and where the
 * grid's counterpart is pinned. Data rather than prose, so the config can
 * check that every spec using the terminal helpers is in exactly one of this
 * and `HOST_TERMINAL_SPECS`.
 */
export const HOST_TERMINAL_LEFT_OUT: ReadonlyMap<string, string> = new Map([
  [
    "terminal-link-clicks.spec.ts",
    "places links by writing into xterm.js and reads its mouse-mode names, buffer and link-gesture bookkeeping; " +
      "the grid's link clicks, withheld presses and covered disclosures are pinned against recorded streams in " +
      "grid-client/grid-attach.spec.ts",
  ],
  [
    "terminal-scroll-freeze.spec.ts",
    "about xterm.js's DOM renderer painting stale rows while scrolled back; the grid's scrolled-back view is pinned " +
      "in grid-client/grid-terminal.spec.ts, and a live flood's scrollback in host-terminals/grid-flood.spec.ts",
  ],
]);
