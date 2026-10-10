// Decisions and notice presentation behind terminal.js's mouse-gesture copying: when a
// completed mouse gesture on the terminal should push the current LOCAL
// xterm selection to the system clipboard (herdr-style copy-on-select), and,
// for a drag a mouse-reporting program took for itself, when to tell the
// user it copied nothing (see "The drag that copies nothing" below). Split out on its own for the
// same reason shift-enter-key.js is (see that file's header) — `node
// --test` runs the EXACT function terminal.js calls, not a hand-copied
// double that could silently drift from what ships.
//
// ## Which selection this covers
//
// A session terminal has TWO independent ideas of "the user selected text".
// `copySelectionOnMouseUp` is about only the second one below; the notice
// functions further down exist because of the first.
//
// - An agent TUI (Claude Code, Codex) that has turned mouse reporting on
//   handles a plain drag ITSELF: xterm.js's SelectionService is disabled
//   for as long as mouse tracking is active (confirmed against the
//   vendored xterm.js — `CoreBrowserTerminal.bindMouse`'s
//   `onProtocolChange` handler calls `this._selectionService.disable()`
//   the moment an app requests mouse events), so a plain drag never becomes
//   a LOCAL selection at all — this page's own selection handlers never
//   fire, and OSC 52 (the vendored `@xterm/addon-clipboard`, wired in
//   terminal.js's `mount()`) is the only path that can reach the system
//   clipboard for it.
// - Holding Shift while dragging FORCES a local xterm selection even with
//   mouse reporting on (`SelectionService.shouldForceSelection` returns
//   `event.shiftKey` on every platform but macOS, checked before the
//   disabled-selection early return; on macOS the same function reads
//   Option instead — see terminal.js's `macOptionClickForcesSelection`
//   comment), exactly like double-click word-select and triple-click
//   line-select, and exactly like ordinary dragging once nothing has mouse
//   reporting on at all. THIS is the selection this module's decision is
//   about.
//
// Both paths are real and neither subsumes the other, which is why this
// fix has two halves living side by side rather than one.
//
// ## Every completed non-empty selection copies — no "did this change" cache
//
// An earlier version of this module compared the gesture's selection
// against the text it last copied, skipping a mouseup that reproduced it —
// intended as a guard against redundant copies, but wrong: it silently
// suppressed a REAL re-copy whenever the SAME text was selected twice in a
// row, which is exactly what happens when something else (another app, a
// different terminal) has overwritten the system clipboard in between and
// the user reselects the text they want back. A copy mechanism that
// sometimes declines to copy what is plainly selected is a worse bug than
// the redundant write it was trying to prevent — a write of identical bytes
// is not observable as a problem, but a copy that silently did not happen
// is.
//
// A plain click must not clobber the clipboard. Without mouse tracking,
// xterm clears the old selection on the press, so no selection remains to
// copy. With tracking, an unforced press belongs to the program and can
// leave an earlier forced selection intact. Its non-empty text is not a
// new copy request: the press must also have allowed local selection.
(function () {
  /**
   * Whether a completed mouse gesture on the terminal should push the
   * current LOCAL xterm selection to the system clipboard.
   *
   * `hasSelection` is `term.hasSelection()` and `selectionText` is
   * `term.getSelection()` — passed separately, rather than inferring
   * "selected" from a non-empty string, because they are xterm's own two
   * independent signals (see its `hasSelection` getter) and this stays a
   * thin decision over both rather than a second guess about their
   * relationship.
   *
   * `trackingAtPress` and `forced` describe the press, not the release:
   * a program can change mouse tracking during the gesture. An unforced
   * press under tracking cannot select locally, even if old text remains.
   *
   * @param {{hasSelection: boolean, selectionText: string, trackingAtPress: boolean, forced: boolean}} state
   * @returns {boolean} true to write `selectionText` to the clipboard now
   */
  function copySelectionOnMouseUp(state) {
    return !!(
      state && (!state.trackingAtPress || state.forced) && state.hasSelection && state.selectionText
    );
  }

  // ## The drag that copies nothing
  //
  // The other half of the split described above has a gap of its own. When
  // a program in the pane has turned mouse reporting on, a plain drag goes
  // to that program; xterm makes no selection and this page has nothing to
  // copy. Programs that copy by themselves answer with an OSC 52 write,
  // which terminal.js forwards to the clipboard. Programs that only draw a
  // highlight (Codex's prompt box, vim with `mouse=a`) leave the user
  // looking at highlighted text and an unchanged clipboard, with no clue
  // why. The functions below decide when terminal.js shows a notice
  // for that case and what it says; the notice is guidance about the
  // program, never a report that a clipboard write failed (SPEC.md keeps
  // clipboard failures silent).

  /**
   * The platforms on which xterm forces a local selection with Option
   * rather than Shift: exactly the `navigator.platform` values the vendored
   * xterm.js treats as Mac (`isMac` in its platform module), whose
   * `SelectionService.shouldForceSelection` reads Option there (with
   * `macOptionClickForcesSelection`, which terminal.js sets) and Shift
   * everywhere else. The notice must name the key that actually works, so
   * this copies xterm's rule rather than guessing from the user agent.
   */
  const XTERM_MAC_PLATFORMS = ["Macintosh", "MacIntel", "MacPPC", "Mac68K"];

  /**
   * The modifier that forces Farhelm's own selection under mouse reporting
   * on this platform.
   *
   * @param {string} platform `navigator.platform`
   * @returns {"Option" | "Shift"}
   */
  function forcingModifier(platform) {
    return XTERM_MAC_PLATFORMS.includes(platform) ? "Option" : "Shift";
  }

  /**
   * How far, in CSS pixels, the pointer must move between press and release
   * for the gesture to count as a drag. Below this it is a click, which a
   * mouse-reporting program treats as a click and nobody expects to copy.
   */
  const DRAG_THRESHOLD_PX = 4;

  /**
   * How long after the release to wait for the program's own OSC 52 copy
   * before deciding it copied nothing. Codex's copy-on-release over its
   * conversation arrives within a frame; this leaves generous room for a
   * slow host without making the notice feel unrelated to the drag.
   */
  const OSC52_GRACE_MS = 1500;

  /**
   * Whether a completed gesture is a plain drag that the program in the
   * pane took for itself, so that nothing was copied unless the program
   * writes OSC 52 within the grace period. terminal.js calls this at the
   * release, then checks the OSC 52 count again after `OSC52_GRACE_MS`.
   *
   * Every condition is something the user would otherwise be told about
   * wrongly: a click is not a drag; a press outside the terminal's screen
   * (the scrollbar) is not a selection attempt; a pane whose program had
   * no mouse tracking at the press makes Farhelm's own selection; a press
   * with the forcing modifier held is Farhelm's selection too, even when it
   * stayed inside one cell and so selected nothing (telling that user to
   * hold the key they are holding would be wrong); and an OSC 52 write since
   * the press means the program copied by itself.
   *
   * `forced` is read from the press event with xterm's own rule (see
   * `pressForcesSelection`), not inferred from the missing selection: under
   * mouse tracking a plain press does not clear an earlier forced
   * selection. That retained text is neither copied nor evidence that this
   * program-owned drag copied anything, so it must not suppress the notice.
   *
   * @param {{button: number, moved: number, onScreen: boolean,
   *          trackingAtPress: boolean, forced: boolean,
   *          hasSelection: boolean, osc52SincePress: boolean}} g `moved` is
   *        the pixel distance between press and release
   * @returns {boolean}
   */
  function dragMayHaveCopiedNothing(g) {
    return !!(
      g &&
      g.button === 0 &&
      g.onScreen &&
      g.moved >= DRAG_THRESHOLD_PX &&
      g.trackingAtPress &&
      !g.forced &&
      !g.osc52SincePress
    );
  }

  /**
   * Whether a press holds the modifier that forces Farhelm's own selection
   * on this platform: xterm's `shouldForceSelection`, Option (`altKey`)
   * where it treats the platform as Mac, Shift everywhere else.
   *
   * @param {{altKey: boolean, shiftKey: boolean}} ev the press event
   * @param {string} platform `navigator.platform`
   * @returns {boolean}
   */
  function pressForcesSelection(ev, platform) {
    return forcingModifier(platform) === "Option" ? !!ev.altKey : !!ev.shiftKey;
  }

  /**
   * Whether an OSC 52 payload is a clipboard WRITE, as opposed to a read
   * query (`c;?`). Only writes mean the program copied; a program that
   * polls the clipboard during the grace period has still copied nothing.
   *
   * @param {string} data the OSC 52 payload after `52;`
   * @returns {boolean}
   */
  function isOsc52Write(data) {
    return !/(^|;)\?$/.test(String(data));
  }

  /**
   * The notice's text. `appHint` is the agent's own copy instruction when
   * the session view knows one (Codex's, from the Rust side), and replaces
   * the generic first sentence; the way to copy with Farhelm instead is the
   * same for every program.
   *
   * @param {{platform: string, appHint: (string|null|undefined)}} opts
   * @returns {string}
   */
  function dragCopyNoticeText(opts) {
    const key = forcingModifier(opts && opts.platform);
    const first =
      (opts && opts.appHint) ||
      "This program handles mouse selection itself, so this drag did not copy anything. " +
        "Use the program's own copy command.";
    return `${first} Or hold ${key} while dragging to select and copy here.`;
  }

  /** The notice remains useful long enough to read without watching the corner. */
  const DRAG_NOTICE_MS = 30_000;

  /**
   * Own the persistent notice's placement and lifetime for one terminal mount.
   *
   * Only the text span changes: replacing the live region or its children
   * would discard the dismiss button and make announcements unreliable.
   * `place` is the shared tooltip placement rule, used with a point-shaped
   * target at the drag's release. Timers and viewport are injected so tests
   * can prove the actual lifetime without waiting thirty seconds.
   *
   * Hiding clears the text as well as the showing class, allowing a repeated
   * notice to be announced. The stylesheet makes the entire hidden subtree
   * inert to the pointer; the dismiss button alone accepts clicks when shown.
   */
  function createDragCopyNotice(el, { place, viewport, setTimeout, clearTimeout }) {
    const text = el.querySelector(".drag-copy-notice-text");
    const dismiss = el.querySelector(".drag-copy-notice-dismiss");
    let timer = null;
    let point = null;

    /** Retire both the visible notice and its deadline, including on teardown. */
    function hide() {
      if (timer !== null) clearTimeout(timer);
      timer = null;
      point = null;
      el.classList.remove("showing");
      text.textContent = "";
    }

    /** Keep the measured box inside the current viewport, including after resize. */
    function reposition() {
      if (point === null) return;
      const size = el.getBoundingClientRect();
      const windowSize = viewport();
      const position = place(
        { left: point.x, right: point.x, top: point.y, bottom: point.y },
        size,
        windowSize,
      );
      el.style.left = `${position.left}px`;
      // Tooltip placement shares horizontal centering and edge clamping.
      // This notice stays above the release rather than flipping below it;
      // at the top edge it overlaps the pointer instead of leaving the window.
      const top = Math.max(4, Math.min(point.y - 6 - size.height, windowSize.height - 4 - size.height));
      el.style.top = `${top}px`;
    }

    /** Prevent the button's press from moving focus or becoming terminal input. */
    function keepFocus(event) {
      event.preventDefault();
      event.stopPropagation();
    }

    /** Dismiss locally; this is never a press in the terminal's gesture handler. */
    function dismissClick(event) {
      keepFocus(event);
      hide();
    }
    dismiss.addEventListener("mousedown", keepFocus);
    dismiss.addEventListener("click", dismissClick);

    return {
      /** Every qualifying drag replaces the previous position and starts a fresh deadline. */
      show(message, release) {
        hide();
        text.textContent = message;
        point = release;
        reposition();
        el.classList.add("showing");
        timer = setTimeout(hide, DRAG_NOTICE_MS);
      },
      hide,
      reposition,
      /** Release listeners and timers with the terminal mount that owns them. */
      dispose() {
        hide();
        dismiss.removeEventListener("mousedown", keepFocus);
        dismiss.removeEventListener("click", dismissClick);
      },
    };
  }

  const api = {
    copySelectionOnMouseUp,
    forcingModifier,
    dragMayHaveCopiedNothing,
    pressForcesSelection,
    isOsc52Write,
    dragCopyNoticeText,
    createDragCopyNotice,
    DRAG_NOTICE_MS,
    DRAG_THRESHOLD_PX,
    OSC52_GRACE_MS,
  };
  if (typeof window !== "undefined") window.farhelmCopyOnSelect = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})();
