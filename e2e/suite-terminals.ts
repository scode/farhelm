// Which kind of terminal the main browser suite's stacks create, and the
// explicit old-terminal set that runs against old terminals.
//
// The ordinary run (`npx playwright test`, the variable below unset) is the
// new-terminal run: start-stack.sh starts both of every stack's supervisors
// with `--new-terminals host`, so every session and tab the specs create is a
// terminal in a terminal host, drawn by the grid component. That is what
// every shipped supervisor creates, and the suite tests it by default.
//
// Old terminals (tmux, drawn by xterm.js) remain for sessions made by earlier
// versions, so the behavior that exists only for them keeps a run of its own:
// `FARHELM_E2E_SUITE_TERMINALS=tmux npx playwright test` starts every stack's
// supervisors with `--new-terminals tmux` and runs only the old-terminal set,
// which is the whole spec files in `OLD_TERMINAL_FILES` and the tests tagged
// `OLD_TERMINAL_TAG` in the files `OLD_TERMINAL_TAGGED` lists. The ordinary
// run leaves both out. A test belongs in the set only when its subject is the
// old terminal's implementation (xterm.js's catch-up buffering, its write
// callbacks and flow control, its parser and DOM) or a behavior only old
// sessions have; a test of product behavior runs on new terminals and is not
// duplicated here.
//
// playwright.config.ts refuses either run when this account is not complete
// (a listed file missing, a file in both lists, a spec using the tag that is
// not listed as tagged, a listed tagged file that uses it nowhere). This file
// has no imports, so scripts/tensorlake/sandbox-tests.ts
// can load it from the checkout it shards without the rest of the suite.

/**
 * The environment variable naming the run's terminals: unset or empty for
 * the ordinary new-terminal run, `tmux` for the old-terminal set. Read by
 * playwright.config.ts and, through the worker processes' inherited
 * environment, by `suiteTerminals()` in tests/helpers/term.ts. Nothing sets
 * it from inside a test. The test recorder scrubs `FARHELM_*` variables, so a
 * recorded old-terminal run passes `--keep-farhelm-env` with this name.
 */
export const SUITE_TERMINALS_ENV = "FARHELM_E2E_SUITE_TERMINALS";

/** The kind of terminal a run's stacks create: `host` (new) or `tmux` (old). */
export type SuiteTerminals = "host" | "tmux";

/**
 * The run's terminal kind from `SUITE_TERMINALS_ENV`'s value. Anything but
 * unset, empty or `tmux` throws rather than falling back to the ordinary run,
 * so a misspelled request for the old-terminal set cannot pass as one.
 */
export function suiteTerminalsFrom(value: string | undefined): SuiteTerminals {
  if (value === undefined || value === "") return "host";
  if (value === "tmux") return "tmux";
  throw new Error(`${SUITE_TERMINALS_ENV} must be unset, empty or "tmux", got "${value}"`);
}

/**
 * The main config's metadata key naming its run's terminal kind (`host` or
 * `tmux`). Global setup and the per-test premise check
 * (tests/helpers/host-premise.ts) read this rather than the variable, which
 * other configs' runners may inherit; it also shows the run's kind in a
 * report.
 */
export const SUITE_TERMINALS_METADATA = "suiteTerminals";

/**
 * The Playwright tag of a test in the old-terminal set inside a spec file
 * whose other tests run on new terminals: `test(title, { tag:
 * OLD_TERMINAL_TAG }, body)`. Its file must be in `OLD_TERMINAL_TAGGED`.
 */
export const OLD_TERMINAL_TAG = "@old-terminal";

/**
 * Matches a test carrying `OLD_TERMINAL_TAG`. Playwright matches a project's
 * `grep` and `grepInvert` against the test's title path followed by its tags,
 * so this matches the tag as a whole word and never a title that merely
 * mentions old terminals.
 */
export const OLD_TERMINAL_GREP = new RegExp(`(^|\\s)${OLD_TERMINAL_TAG}(?=\\s|$)`);

/**
 * Spec files (relative to tests/) every test of which is about old
 * terminals, with why and where new terminals' counterpart is pinned. Only
 * the old-terminal run has them.
 */
export const OLD_TERMINAL_FILES: ReadonlyMap<string, string> = new Map([
  [
    "terminal-link-clicks.spec.ts",
    "places links by writing into xterm.js and reads its mouse-mode names, buffer and link-gesture bookkeeping; " +
      "the grid's link clicks, withheld presses and covered disclosures are pinned against recorded streams in " +
      "grid-client/grid-attach.spec.ts",
  ],
  [
    "terminal-scroll-freeze.spec.ts",
    "about xterm.js's DOM renderer painting stale rows while scrolled back; the grid's scrolled-back view is pinned " +
      "in grid-client/grid-terminal.spec.ts, and a live flood's scrollback in tests/grid-flood.spec.ts",
  ],
]);

/**
 * Spec files (relative to tests/) holding tests tagged `OLD_TERMINAL_TAG`
 * beside tests of the ordinary run. Each tagged test says in place why it is
 * about old terminals and where new terminals' counterpart is pinned.
 */
export const OLD_TERMINAL_TAGGED: ReadonlySet<string> = new Set([
  "terminal-clipboard.spec.ts",
  "terminal-files.spec.ts",
  "terminal-flood.spec.ts",
  "terminal-links.spec.ts",
  "terminal-migration.spec.ts",
  "terminal-reconnect.spec.ts",
  "terminal-replay-rename.spec.ts",
  "terminal-tabs.spec.ts",
  "terminal-unicode.spec.ts",
  "terminal.spec.ts",
]);
