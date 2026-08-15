// The reusable half of driving a REAL terminal agent (Claude Code today;
// any agent sharing the same shape tomorrow) through this suite's
// browser-driven terminal, plus the fake-agent-compatible primitives that
// let its plumbing be exercised in CI without vendor credentials
// (PLAN_M6_5.md item 3).
//
// This is a helper MODULE, not a spec file, and that is deliberate: every
// other spec in this suite duplicates its own small helpers rather than
// importing across files (terminal.spec.ts exports nothing, and
// mouse-modes.spec.ts's own header explains why — new specs are meant to
// start clean). This one is the named exception the plan calls for: the
// lessons below are expensive enough to relearn that a copy-per-spec
// would defeat the point, so this module exports them for
// real-agent.spec.ts to import directly.
//
// Every function here distills one of three lessons the first
// agent-driven smoke test paid for by hand:
//
//   (a) A vendor agent's own onboarding UI — Claude Code's folder-trust
//       dialog — sits in front of the terminal it owns and must be
//       detected and dismissed before ANY prompt is typed, or the prompt
//       lands on a modal that eats it silently. See `waitUntilAgentReady`.
//   (b) Typing a prompt and its submitting Enter in the same burst risks
//       the agent's fast-typing paste heuristic reading the whole thing
//       as a paste and swallowing the Enter into the composer instead of
//       submitting, rather than treating it as a submission (PLAN_M6_5.md
//       item 3's own finding, driving Claude Code through this suite's
//       browser path; farhelm's Rust-side real-agent capture test hit the
//       same class of bug against Codex over a raw PTY — see
//       `real_agent_capture.rs` — so treating the fix as universal rather
//       than Claude-specific costs nothing and closes the whole bug class
//       for any agent this helper drives). See `submitPrompt`.
//   (c) A reply is detected by a MARKER the agent's own output produces,
//       never by "it went quiet for N ms" — an agent can go quiet mid
//       tool-call or mid-thinking and then resume, which a silence
//       heuristic cannot tell apart from "done replying". See
//       `waitForReplyMarker`.
//
// What real-agent.spec.ts's CI-runnable tests actually pin (a 14-reviewer
// pass over the first version of this file is the reason this list is
// this precise rather than a general "the plumbing works" claim):
//   - `waitUntilAgentReady` still honors its deadline when a dialog
//     marker never clears, rather than looping Enter forever (the
//     bypass a `continue` before the deadline check used to allow).
//   - `waitForReplyMarker` genuinely stays pending — through unrelated
//     echoed output AND a quiet gap — until its marker is actually
//     produced, and rejects rather than hangs when a marker never
//     arrives at all.
//   - `submitPrompt`'s settle gap between typing and Enter is a REAL
//     elapsed wait, not a no-op, observed through its own event-recorder
//     seam.
// None of this proves lesson (a)'s dialog-text matching against a real
// vendor UI (the fake agent has no onboarding dialog to dismiss) or
// lesson (b)'s actual defeat of a real paste heuristic (the fake agent
// has none) — both stay real-agent.spec.ts's env-gated leg's job, and
// stay manual otherwise per SPEC_impl.md's testing section.
import { Page, expect } from "@playwright/test";

/**
 * The markers one agent kind needs: substrings that mean "an onboarding
 * dialog is blocking input, dismiss it with Enter before anything else",
 * and the substring that means "booted and accepting input". Modeled as a
 * plain interface (rather than an enum of known agents) because the
 * CI-runnable fake-agent exercise needs to supply its OWN trivial pair —
 * empty dialog markers, `"FAKE-AGENT READY"` as ready — through the exact
 * same code path a real agent's markers use, which is the whole point of
 * that exercise (see real-agent.spec.ts).
 */
export interface AgentMarkers {
  /**
   * Substrings that mean a blocking onboarding dialog is on screen,
   * matched against the FULL BUFFER (see `waitUntilAgentReady`). Empty
   * for an agent/configuration known not to show one — the fake agent,
   * which has no onboarding UI at all, or a real agent whose workspace is
   * already recorded as trusted. NOT "a real agent used before" in
   * general: Claude Code's trust is PER-WORKSPACE, not global, so having
   * run it once elsewhere proves nothing about THIS cwd. `waitUntilAgentReady`
   * then degrades to "poll for the ready marker", which is exactly what
   * the fake-agent leg exercises.
   */
  trustDialogMarkers: string[];
  /** Substring proving the agent is booted and reading input. */
  readyMarker: string;
}

/**
 * Claude Code's own markers, audited live against v2.1.220
 * (`real_agent_capture.rs`'s
 * `real_claude_session_captures_its_conversation_identity`). Kept in sync
 * with that test deliberately: a vendor UI-text change would need both
 * updated together, and duplicating the audited values here rather than
 * importing them is unavoidable — that test lives in the Rust crate, this
 * helper in the TS suite, and nothing links the two builds.
 *
 * `readyMarker` is `"Claude Code v"`, not the more obvious `"Claude
 * Code"` — the Rust test's own first real run matched the latter against
 * the trust dialog's own body text ("Claude Code'll be able to read,
 * edit, and execute files here"), which made its wait loop declare
 * victory while the dialog was still open and type the prompt straight
 * into it. `"Claude Code v"` appears only in the startup banner. This is
 * also exactly why `waitUntilAgentReady` checks dialog markers BEFORE the
 * ready marker on every poll rather than trusting a ready marker to never
 * overlap dialog text: a future vendor UI change could reintroduce the
 * overlap, and the check order — not this specific string — is what
 * stays safe even then.
 */
export const CLAUDE_CODE_MARKERS: AgentMarkers = {
  trustDialogMarkers: ["Accessing workspace", "Do you trust"],
  readyMarker: "Claude Code v",
};

/**
 * Read xterm's buffer over `[start, end)` (full buffer when `mode` is
 * `"full"`, the current screenful alone when `"viewport"`) and
 * reconstruct LOGICAL lines from it, joining a soft-wrapped row onto its
 * predecessor rather than treating the wrap as a line break.
 *
 * That reconstruction is not cosmetic. xterm splits a paragraph into
 * several buffer ROWS purely because of the terminal's column width,
 * flagging each continuation row `isWrapped`; a naive `\n`-join of every
 * row (an earlier version of this module did exactly that) turns each
 * wrap point into an invisible line break wherever the terminal happened
 * to be narrow that run — which can slice a marker in half across the
 * two halves and leave it unmatchable by either mode. `translateToString`
 * strips formatting (color codes, etc.) without consuming columns, so
 * wrap points land purely on VISIBLE character count, matching what a
 * person looking at the pane would actually see wrap.
 *
 * The single shared implementation (rather than one copy per mode) is
 * what keeps `termText` and `viewportText` from silently drifting apart
 * on this — `page.evaluate` ships a self-contained function to the
 * browser, so the mode has to be a serializable parameter branched on
 * INSIDE the evaluated function rather than two separate closures calling
 * a shared TS helper (which the browser realm cannot see at all).
 */
async function bufferText(page: Page, mode: "full" | "viewport"): Promise<string> {
  return page.evaluate((mode) => {
    const term = (window as any).__farhelmTerm;
    if (!term) return "";
    const buf = term.buffer.active;
    const start = mode === "viewport" ? buf.viewportY : 0;
    const end = mode === "viewport" ? buf.viewportY + term.rows : buf.length;
    const logicalLines: string[] = [];
    for (let i = start; i < end; i++) {
      const line = buf.getLine(i);
      const text = line?.translateToString(true) ?? "";
      if (line?.isWrapped && logicalLines.length > 0) {
        // A continuation of the previous ROW's own line, not a new one:
        // append onto what is already there instead of starting a fresh
        // entry.
        logicalLines[logicalLines.length - 1] += text;
      } else {
        logicalLines.push(text);
      }
    }
    return logicalLines.join("\n");
  }, mode);
}

/**
 * Full text content of the terminal buffer (scrollback + viewport). Used
 * by both `waitForReplyMarker` and `waitUntilAgentReady`, for two
 * distinct but related reasons a poll must not be fooled by content
 * moving out of the current screenful: a reply marker must stay found
 * even after later output has pushed it off screen ("did a reply arrive"
 * does not stop being true just because the poll ran late), and a dialog
 * or ready marker must stay found even after the post-mount font-swap
 * refit RE-WRAPS everything already printed and leaves it at a different
 * row than it went in at (see `waitUntilAgentReady`'s own doc for the
 * concrete CI failure that motivated the second case).
 *
 * Not exported (no use exists outside this module) — duplicated from
 * terminal.spec.ts's identical helper rather than imported, per this
 * suite's testing decision that specs stay self-contained.
 */
async function termText(page: Page): Promise<string> {
  return bufferText(page, "full");
}

/**
 * The currently VISIBLE screenful only — `term.rows` lines starting at
 * `buffer.viewportY` — as opposed to `termText`'s full scrollback.
 *
 * NOT used for matching anymore (`waitUntilAgentReady` polls the full
 * buffer for both its dialog and ready markers — see that function's own
 * doc for why viewport-only matching turned out to be the wrong fix for
 * the "stale text lingering in scrollback" problem it was built to solve).
 * This function survives purely as the timeout error's diagnostic dump: a
 * short, CURRENT-screen rendering is a more useful thing to paste into a
 * bug report than a full scrollback dump that could run to hundreds of
 * lines by the time a real agent has been running a while.
 */
async function viewportText(page: Page): Promise<string> {
  return bufferText(page, "viewport");
}

/**
 * Lesson (a). Poll the pane until `markers.readyMarker` appears on
 * screen, dismissing any onboarding dialog with a bare Enter along the
 * way. Resolves with how many dialogs were dismissed, so a caller that
 * KNOWS a dialog must appear (a fresh, untrusted scratch directory) can
 * assert it actually did rather than trusting readiness alone — a ready
 * marker that shows up without ever having matched a dialog marker could
 * mean either "there was truly nothing to dismiss" or "the dialog check
 * missed it", and only the caller's own context can tell those apart
 * (`opts.minDialogDismissals` makes that assertion part of the wait
 * itself; see real-agent.spec.ts's real-agent leg for the case that
 * needs it).
 *
 * The deadline is checked FIRST on every iteration, before either marker
 * branch — this is load-bearing, not stylistic. An earlier version
 * checked it only on the "no marker matched at all" path, so a dialog
 * marker that never clears (a genuine bug, or this function pointed at
 * the wrong text) looped Enter forever with the promised timeout, and
 * its pane diagnostic, never firing. Checking first makes the deadline
 * unconditional: however the loop got here, time runs out.
 *
 * Both marker kinds are matched against the FULL BUFFER (`termText`), not
 * the live viewport alone — the opposite of an earlier version of this
 * function, and changed for a real, CI-observed reason: terminal.js's
 * post-mount font swap (its `document.fonts.load(...).then(...)` block)
 * re-fits the terminal once JetBrains Mono actually loads, and that
 * second `fit.fit()` re-measures the cell size and can change `cols`,
 * which makes xterm RE-WRAP every row already printed to the new column
 * width. Content printed early — a startup banner, a ready marker — can
 * come out of that reflow at a different row than it went in at, and
 * nothing guarantees it stays inside the CURRENT `[viewportY, viewportY +
 * rows)` window: this is exactly what timed out `spawn.spec.ts`'s own
 * ready-marker wait on both engines, with an effectively blank rendered
 * viewport and the marker sitting in scrollback above it. Reading the
 * full buffer instead makes the wait immune to where the reflow happened
 * to leave the marker.
 *
 * This does reopen the specific risk `viewportText`'s own retired doc
 * described — a dialog marker's text lingering in scrollback after real
 * dismissal, or a ready marker's banner staying matchable long after a
 * LATER dialog has appeared — but the failure modes are not symmetric.
 * Both trust-dialog marker strings ("Accessing workspace", "Do you
 * trust") are dialog-specific UI copy a normally-running agent never
 * prints again, and Claude Code's TUI redraws its current screen on every
 * frame (including after a resize) rather than leaving a stale dialog
 * sitting unrefreshed — so a genuinely dismissed dialog's marker text
 * reappearing in a LATER poll is not the expected case. Where it could
 * still happen, the cost is one redundant Enter into a running prompt
 * (harmless — an empty submit an agent's own composer ignores), against a
 * wait that used to hang past its deadline whenever a reflow knocked a
 * legitimately-present marker out of the viewport. The timeout error
 * below still renders the live viewport, not the full buffer, as its
 * diagnostic dump — see `viewportText`'s own doc for why that stays the
 * more useful thing to paste into a bug report.
 *
 * The dialog's Enter is sent alone, with no prompt text ahead of it, so
 * lesson (b)'s paste-heuristic risk does not apply here — that risk is
 * specifically about a burst of TEXT-then-Enter, and a bare keypress has
 * nothing in it to be mistaken for a paste. A settle delay after it still
 * matters, though: `dialogRetryMs` gives the dialog time to actually
 * redraw as dismissed before the next poll, rather than hammering Enter
 * at a modal still mid-transition and risking a second Enter landing on
 * whatever comes up next.
 *
 * Throws with the rendered viewport on timeout, the same diagnostic
 * `real_agent_capture.rs`'s own wait loop relies on — a failure that only
 * ever reports "timed out" leaves the next person reproducing it from
 * scratch just to see what the agent actually printed.
 */
export async function waitUntilAgentReady(
  page: Page,
  markers: AgentMarkers,
  opts: {
    timeoutMs?: number;
    pollMs?: number;
    dialogRetryMs?: number;
    /**
     * Require at least this many dialog dismissals before a ready marker
     * is accepted. 0 (the default) accepts readiness on its own, which
     * is what every caller with no dialog to dismiss (the fake agent)
     * needs.
     */
    minDialogDismissals?: number;
  } = {},
): Promise<{ dialogDismissals: number }> {
  // 30s, not the 60s the project's own default test timeout
  // (playwright.config.ts) uses: a helper default equal to the OUTER
  // timeout would let Playwright's generic "test timed out" fire first,
  // silently discarding this function's own pane diagnostic — the exact
  // failure mode its error message exists to prevent.
  const timeoutMs = opts.timeoutMs ?? 30_000;
  const pollMs = opts.pollMs ?? 500;
  const dialogRetryMs = opts.dialogRetryMs ?? 2_000;
  const minDialogDismissals = opts.minDialogDismissals ?? 0;
  const deadline = Date.now() + timeoutMs;
  let dialogDismissals = 0;
  for (;;) {
    if (Date.now() > deadline) {
      const text = await viewportText(page);
      throw new Error(
        `timed out waiting for ready marker ${JSON.stringify(markers.readyMarker)} ` +
          `(dismissed ${dialogDismissals} dialog(s); needed at least ${minDialogDismissals}); ` +
          `rendered viewport:\n${text}`,
      );
    }
    const text = await termText(page);
    if (markers.trustDialogMarkers.some((marker) => text.includes(marker))) {
      // Explicit focus before the dismissal keystroke: unlike
      // `submitPrompt`, this function has not typed anything yet by this
      // point, so whatever element the PAGE happens to have focus on is
      // unspecified — an Enter sent to the wrong element would silently
      // do nothing, and the loop would spin until its deadline with no
      // sign of why.
      await page.locator("#terminal").click();
      await page.keyboard.press("Enter");
      dialogDismissals += 1;
      await page.waitForTimeout(dialogRetryMs);
      continue;
    }
    if (text.includes(markers.readyMarker) && dialogDismissals >= minDialogDismissals) {
      return { dialogDismissals };
    }
    await page.waitForTimeout(pollMs);
  }
}

/**
 * One point in `submitPrompt`'s own timeline, handed to an optional
 * observer. Exists purely so a test can prove the settle gap is a REAL
 * elapsed wait rather than a no-op — the seam only OBSERVES timestamps
 * `submitPrompt` was already going to produce; it changes nothing about
 * what gets typed, waited, or sent.
 */
export interface SubmitPromptEvent {
  event: "type-complete" | "settle-elapsed" | "enter-sent";
  atMs: number;
}

/**
 * Lesson (b). Focus the terminal, type `text`, let it settle, and only
 * THEN press Enter as its own keystroke.
 *
 * `page.keyboard.type` already sends one key event per character rather
 * than one paste event, which is what makes the swallow bug possible in
 * the first place: an agent's TUI decides "this looks like a paste" from
 * the ARRIVAL PACING of the bytes, not from how the browser generated
 * them, and a fast enough sequence of keystrokes reads the same as a real
 * clipboard paste to that heuristic. Text immediately followed by `\r` in
 * that same burst is exactly the shape a paste has (bracketed-paste
 * content commonly ends in a trailing newline), so the fix is not to type
 * slower — it is to make Enter arrive OUTSIDE the burst entirely, after a
 * `settleMs` gap the heuristic reads as "typing stopped; this was not one
 * paste".
 *
 * `onEvent`, when given, is called at each of the three stages with a
 * timestamp — a pacing seam for tests (see real-agent.spec.ts's own
 * pacing test), never a way to alter the wait: nothing here changes
 * behavior when `onEvent` is omitted, and the callback cannot affect the
 * awaited delays or their order.
 */
export async function submitPrompt(
  page: Page,
  text: string,
  settleMs = 1_000,
  onEvent?: (event: SubmitPromptEvent) => void,
): Promise<void> {
  await page.locator("#terminal").click();
  await page.keyboard.type(text);
  onEvent?.({ event: "type-complete", atMs: Date.now() });
  await page.waitForTimeout(settleMs);
  onEvent?.({ event: "settle-elapsed", atMs: Date.now() });
  await page.keyboard.press("Enter");
  onEvent?.({ event: "enter-sent", atMs: Date.now() });
}

/**
 * Which markers `waitForReplyMarker` has already waited out successfully,
 * per page — the state the reuse precondition below checks against.
 *
 * A `WeakMap` keyed on `Page` rather than a plain module-level `Set`:
 * this suite's own tests run many independent sessions (and, in a
 * multi-project run, chromium and webkit share this module but never a
 * `Page`), and a bare `Set` would treat two different pages' identical
 * marker strings as the same history. Keying on `Page` also means a
 * closed page's entry becomes garbage the next time it runs, with no
 * explicit teardown needed.
 */
const consumedReplyMarkers = new WeakMap<Page, Set<string>>();

/**
 * Lesson (c). Poll the FULL buffer (`termText`) until `marker` appears as
 * a substring. Never waits on silence: an agent going quiet mid-tool-call
 * or mid-thinking is indistinguishable from "done replying" to a timer,
 * so the only signal this suite trusts is content the agent's own reply
 * actually produced — proven pending through unrelated output and a
 * quiet gap, and rejecting rather than hanging when the marker never
 * shows at all, by real-agent.spec.ts's own regression tests.
 *
 * CONTRACT: `marker` must be FRESH — never passed to this function twice
 * for the SAME page after the first wait for it already succeeded.
 * Reusing one would resolve the second wait instantly on the very
 * occurrence the first wait already consumed, proving nothing about
 * whatever happened in between — silently wrong rather than loudly
 * wrong. Every caller so far has used unique, timestamped markers and so
 * never hit this; the precondition (tracked per page in
 * `consumedReplyMarkers`) exists so a future caller does not have to get
 * that right purely by convention. Violating it throws immediately,
 * before any polling begins.
 *
 * The check is reuse-across-waits specifically, NOT "is the marker
 * already in the buffer right now" — a version of this function tried
 * that first and it was wrong: this suite's own fake agent (and a fast
 * real completion) routinely satisfies a marker before the calling code
 * even reaches this function, which is the NORMAL, successful case for
 * every test in this file, not a violation of anything.
 *
 * String-only, deliberately: an earlier version also accepted a `RegExp`
 * for "anything shape-based", but nothing in this suite ever needed that,
 * and a `RegExp` argument dragged its own statefulness (`g`/`y` flags
 * advance `lastIndex` across calls) into what is otherwise a pure polling
 * predicate — a footgun with no call site to justify it.
 *
 * For a real agent whose reply text is otherwise unpredictable (an LLM,
 * not a fixture), the caller manufactures a marker by asking the agent to
 * COMPUTE one rather than by naming it directly in the prompt — see
 * real-agent.spec.ts's own prompt for why the marker must never appear in
 * the prompt text itself.
 */
export async function waitForReplyMarker(
  page: Page,
  marker: string,
  timeoutMs = 15_000,
): Promise<void> {
  let consumed = consumedReplyMarkers.get(page);
  if (!consumed) {
    consumed = new Set();
    consumedReplyMarkers.set(page, consumed);
  }
  if (consumed.has(marker)) {
    throw new Error(
      `waitForReplyMarker precondition violated: ${JSON.stringify(marker)} was already waited ` +
        `for and satisfied by an earlier call on this page. Markers must be FRESH per wait — ` +
        `reusing one resolves on the SAME occurrence a previous wait already consumed, not on ` +
        `anything new.`,
    );
  }
  await expect
    .poll(
      async () => (await termText(page)).includes(marker),
      { timeout: timeoutMs, message: `waiting for reply marker ${JSON.stringify(marker)}` },
    )
    .toBe(true);
  consumed.add(marker);
}
