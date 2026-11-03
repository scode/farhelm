// The grid client's input side: which keyboard, mouse, wheel and focus
// events become which input messages. Pure, no DOM.
//
// A grid terminal does not encode input. It reports what the user did as
// `key`, `text`, `mouse` and `focus` messages, and the supervisor, which
// knows the program's modes (application cursor keys, the mouse protocol
// and encoding), turns them into the bytes the program reads. What stays
// here is the part only the page can decide: whether a key event is one
// the terminal sends at all, and in which form.
//
// ## Keys: the same split xterm.js makes today
//
// Today's terminal (the vendored xterm.js 6.0.0) decides at keydown whether
// a key produces a sequence: `evaluateKeyboardEvent` maps the event to one,
// and `CoreBrowserTerminal._keyDown` sends it and cancels the event, or
// leaves the event alone so the character arrives through the browser's
// own text input (`_keyPress`/`_inputEvent`) instead. Option on a Mac,
// dead keys and AltGr layouts depend on that second path.
//
// `keydownAction` reproduces that decision exactly, using a port of
// `evaluateKeyboardEvent` (`xtermKeyboardResult`) whose output the unit
// tests compare with the vendored bundle's own function over every key
// code and modifier combination. Where xterm.js would send a sequence, the
// grid sends a `key` message and the supervisor computes the same sequence
// from the same function with the program's modes; where xterm.js would
// wait for text, the grid waits for the input event and sends `text`.
// Because the decision whether a sequence exists does not depend on the
// program's modes, the page can make it without knowing them.
//
// Shift+Enter needs nothing special here: it is a `key` message like any
// Enter, and the supervisor sends ESC CR for it in one write (SPEC.md,
// "Terminal experience").
//
// ## Mouse and wheel
//
// Mouse events become `mouse` messages in cell coordinates, sent only for
// what the program's mouse mode asks for (`state.mouse`), with the same
// cell rounding, clamping and move deduplication as xterm.js's mouse
// service. Wheel steps follow xterm.js's `consumeWheelEvent`, including its
// pixel accumulation, so a trackpad scrolls a program at today's rate.
(function (root) {
  "use strict";

  const MAX_INPUT_MESSAGE = 64 * 1024;
  const MAX_TEXT_BYTES = 16 * 1024;
  const MAX_KEY_FIELD_BYTES = 64;

  const MODS = Object.freeze({ SHIFT: 1, ALT: 2, CTRL: 4, META: 8 });

  // xterm.js's C0 names, as `evaluateKeyboardEvent` uses them.
  const C0 = Object.freeze({
    NUL: "\x00",
    BS: "\b",
    HT: "\t",
    CR: "\r",
    ESC: "\x1b",
    FS: "\x1c",
    GS: "\x1d",
    US: "\x1f",
    DEL: "\x7f",
  });

  // xterm.js's `KEYCODE_KEY_MAPPINGS`: the unshifted and shifted character
  // of each US-layout key that Alt+key encodes as ESC + character.
  const KEYCODE_KEY_MAPPINGS = Object.freeze({
    48: ["0", ")"],
    49: ["1", "!"],
    50: ["2", "@"],
    51: ["3", "#"],
    52: ["4", "$"],
    53: ["5", "%"],
    54: ["6", "^"],
    55: ["7", "&"],
    56: ["8", "*"],
    57: ["9", "("],
    186: [";", ":"],
    187: ["=", "+"],
    188: [",", "<"],
    189: ["-", "_"],
    190: [".", ">"],
    191: ["/", "?"],
    192: ["`", "~"],
    219: ["[", "{"],
    220: ["\\", "|"],
    221: ["]", "}"],
    222: ["'", '"'],
  });

  // Result types, as xterm.js numbers them.
  const KEY_RESULT = Object.freeze({ SEND_KEY: 0, SELECT_ALL: 1, PAGE_UP: 2, PAGE_DOWN: 3 });

  /** The modifier bit set the input messages carry (and xterm.js computes). */
  function modifiers(ev) {
    return (ev.shiftKey ? MODS.SHIFT : 0) | (ev.altKey ? MODS.ALT : 0) | (ev.ctrlKey ? MODS.CTRL : 0) |
      (ev.metaKey ? MODS.META : 0);
  }

  /**
   * A port of the vendored xterm.js 6.0.0's `evaluateKeyboardEvent`:
   * `{type, cancel, key}`, where `key` is the sequence xterm.js would send
   * for this keydown (undefined when it sends none at keydown).
   *
   * Kept line for line with the original's branches rather than
   * simplified, because its whole value is being the same function; the
   * unit tests hold it to the vendored bundle. The supervisor's encoder
   * makes the same computation with the program's real modes; the page
   * only needs `type`, `cancel` and whether `key` exists.
   */
  function xtermKeyboardResult(ev, applicationCursorMode, isMac, macOptionIsMeta) {
    const result = { type: KEY_RESULT.SEND_KEY, cancel: false, key: undefined };
    const mods = modifiers(ev);
    const cursor = (letter) => (mods ? `${C0.ESC}[1;${mods + 1}${letter}` : applicationCursorMode ? `${C0.ESC}O${letter}` : `${C0.ESC}[${letter}`);
    switch (ev.keyCode) {
      case 0:
        if (ev.key === "UIKeyInputUpArrow") result.key = applicationCursorMode ? `${C0.ESC}OA` : `${C0.ESC}[A`;
        else if (ev.key === "UIKeyInputLeftArrow") result.key = applicationCursorMode ? `${C0.ESC}OD` : `${C0.ESC}[D`;
        else if (ev.key === "UIKeyInputRightArrow") result.key = applicationCursorMode ? `${C0.ESC}OC` : `${C0.ESC}[C`;
        else if (ev.key === "UIKeyInputDownArrow") result.key = applicationCursorMode ? `${C0.ESC}OB` : `${C0.ESC}[B`;
        break;
      case 8:
        result.key = ev.ctrlKey ? C0.BS : C0.DEL;
        if (ev.altKey) result.key = C0.ESC + result.key;
        break;
      case 9:
        if (ev.shiftKey) {
          result.key = `${C0.ESC}[Z`;
          break;
        }
        result.key = C0.HT;
        result.cancel = true;
        break;
      case 13:
        result.key = ev.altKey ? C0.ESC + C0.CR : C0.CR;
        result.cancel = true;
        break;
      case 27:
        result.key = C0.ESC;
        if (ev.altKey) result.key = C0.ESC + C0.ESC;
        result.cancel = true;
        break;
      case 37:
        if (ev.metaKey) break;
        result.key = cursor("D");
        break;
      case 39:
        if (ev.metaKey) break;
        result.key = cursor("C");
        break;
      case 38:
        if (ev.metaKey) break;
        result.key = cursor("A");
        break;
      case 40:
        if (ev.metaKey) break;
        result.key = cursor("B");
        break;
      case 45:
        if (!ev.shiftKey && !ev.ctrlKey) result.key = `${C0.ESC}[2~`;
        break;
      case 46:
        result.key = mods ? `${C0.ESC}[3;${mods + 1}~` : `${C0.ESC}[3~`;
        break;
      case 36:
        result.key = cursor("H");
        break;
      case 35:
        result.key = cursor("F");
        break;
      case 33:
        if (ev.shiftKey) result.type = KEY_RESULT.PAGE_UP;
        else if (ev.ctrlKey) result.key = `${C0.ESC}[5;${mods + 1}~`;
        else result.key = `${C0.ESC}[5~`;
        break;
      case 34:
        if (ev.shiftKey) result.type = KEY_RESULT.PAGE_DOWN;
        else if (ev.ctrlKey) result.key = `${C0.ESC}[6;${mods + 1}~`;
        else result.key = `${C0.ESC}[6~`;
        break;
      case 112:
      case 113:
      case 114:
      case 115: {
        const letter = "PQRS"[ev.keyCode - 112];
        result.key = mods ? `${C0.ESC}[1;${mods + 1}${letter}` : `${C0.ESC}O${letter}`;
        break;
      }
      case 116:
      case 117:
      case 118:
      case 119:
      case 120:
      case 121:
      case 122:
      case 123: {
        const code = [15, 17, 18, 19, 20, 21, 23, 24][ev.keyCode - 116];
        result.key = mods ? `${C0.ESC}[${code};${mods + 1}~` : `${C0.ESC}[${code}~`;
        break;
      }
      default:
        if (ev.ctrlKey && !ev.shiftKey && !ev.altKey && !ev.metaKey) {
          // Ctrl alone: the C0 control characters.
          if (ev.keyCode >= 65 && ev.keyCode <= 90) result.key = String.fromCharCode(ev.keyCode - 64);
          else if (ev.keyCode === 32) result.key = C0.NUL;
          else if (ev.keyCode >= 51 && ev.keyCode <= 55) result.key = String.fromCharCode(ev.keyCode - 51 + 27);
          else if (ev.keyCode === 56) result.key = C0.DEL;
          else if (ev.keyCode === 219) result.key = C0.ESC;
          else if (ev.keyCode === 220) result.key = C0.FS;
          else if (ev.keyCode === 221) result.key = C0.GS;
        } else if ((!isMac || macOptionIsMeta) && ev.altKey && !ev.metaKey) {
          // Alt as Meta: ESC + the key's character.
          const mapping = KEYCODE_KEY_MAPPINGS[ev.keyCode];
          const key = mapping && mapping[ev.shiftKey ? 1 : 0];
          if (key) result.key = C0.ESC + key;
          else if (ev.keyCode >= 65 && ev.keyCode <= 90) {
            let ch = String.fromCharCode(ev.ctrlKey ? ev.keyCode - 64 : ev.keyCode + 32);
            if (ev.shiftKey) ch = ch.toUpperCase();
            result.key = C0.ESC + ch;
          } else if (ev.keyCode === 32) result.key = C0.ESC + (ev.ctrlKey ? C0.NUL : " ");
          else if (ev.key === "Dead" && typeof ev.code === "string" && ev.code.startsWith("Key")) {
            let ch = ev.code.slice(3, 4);
            if (!ev.shiftKey) ch = ch.toLowerCase();
            result.key = C0.ESC + ch;
            result.cancel = true;
          }
        } else if (isMac && !ev.altKey && !ev.ctrlKey && !ev.shiftKey && ev.metaKey) {
          if (ev.keyCode === 65) result.type = KEY_RESULT.SELECT_ALL;
        } else if (ev.key && !ev.ctrlKey && !ev.altKey && !ev.metaKey && ev.keyCode >= 48 && ev.key.length === 1) {
          // A key that types exactly one character.
          result.key = ev.key;
        } else if (ev.key && ev.ctrlKey) {
          if (ev.key === "_") result.key = C0.US;
          if (ev.key === "@") result.key = C0.NUL;
        }
        break;
    }
    return result;
  }

  // The `navigator.platform` values the vendored xterm.js treats as a Mac
  // (its `Browser.isMac`), the same list copy-on-select.js uses for the
  // forcing modifier.
  const MAC_PLATFORMS = ["Macintosh", "MacIntel", "MacPPC", "Mac68K"];

  /**
   * Platform names as xterm.js's `Browser` module reads them: exact
   * `navigator.platform` values, not a pattern, because the supervisor's
   * encoder decides Mac-only encodings from the `mac` flag this sets on key
   * messages, and both sides must agree with xterm.js on which viewers are
   * Macs (an iPad's "iPad" or a "MacSomething" are not).
   */
  function platformOf(platform) {
    const name = typeof platform === "string" ? platform : "";
    return {
      isMac: MAC_PLATFORMS.includes(name),
      isWindows: ["Windows", "Win16", "Win32", "WinCE"].includes(name),
    };
  }

  /**
   * xterm.js's `_isThirdLevelShift` for a keydown: Option on a Mac (with
   * `macOptionIsMeta` off, as Farhelm leaves it) or AltGr on Windows,
   * composing a character the text-input path delivers. Not for the arrow
   * and editing keys below key code 48.
   */
  function isThirdLevelShift(ev, platform) {
    const { isMac, isWindows } = platformOf(platform);
    const altGraph = typeof ev.getModifierState === "function" && ev.getModifierState("AltGraph");
    const third = (isMac && ev.altKey && !ev.ctrlKey && !ev.metaKey) ||
      (isWindows && ev.altKey && ev.ctrlKey && !ev.metaKey) || (isWindows && altGraph);
    return !!third && (!ev.keyCode || ev.keyCode > 47);
  }

  /**
   * What the grid does with one keydown, mirroring
   * `CoreBrowserTerminal._keyDown` (with `macOptionIsMeta` and screen-reader
   * mode off, as Farhelm configures xterm.js):
   *
   * - `"send"`: send a `key` message and cancel the event;
   * - `"pass"`: leave the event to the browser, whose text input (if any)
   *   arrives as an input event and becomes a `text` message; `cancel`
   *   says whether to prevent the default anyway (xterm.js does for some
   *   keys it does not send, such as a dead key under Alt);
   * - `"page-up"`/`"page-down"`: Shift+PageUp/Down, which scroll the
   *   terminal's own view and never reach the program;
   * - `"select-all"`: Cmd+A on a Mac, which selects locally;
   * - `"ignore"`: an input method owns the key.
   *
   * The supervisor's encoder (built from `evaluateKeyboardEvent`) assumes
   * it receives only keydowns xterm.js would send, so `_keyDown`'s filters
   * apply exactly: a third-level shift (Option on a Mac, so Option+F1-F12
   * and Option with the iOS arrow keys too; Ctrl+Alt or AltGraph on
   * Windows) is left to text input, as are capital letters and the key
   * after a dead key; an input method's keys are ignored. The unit tests
   * hold this to the vendored `_keyDown` over the whole key space.
   *
   * `deadKeyPending` is xterm.js's `_unprocessedDeadKey`: once a dead key
   * (or AltGraph) has gone down, the next key that would send a sequence is
   * left to text input instead, so the composed character is what arrives.
   * Pass the returned value back in on the next keydown, and reset it to
   * false when text input sends something, as xterm.js's text handlers do.
   */
  function keydownAction(ev, platform, deadKeyPending) {
    if (ev.isComposing || ev.keyCode === 229) return { action: "ignore", cancel: false, deadKeyPending };
    let pending = !!deadKeyPending || ev.key === "Dead" || ev.key === "AltGraph";
    const { isMac } = platformOf(platform);
    const result = xtermKeyboardResult(ev, false, isMac, false);
    const outcome = (action, cancel) => ({ action, cancel, deadKeyPending: pending });
    if (result.type === KEY_RESULT.PAGE_UP) return outcome("page-up", true);
    if (result.type === KEY_RESULT.PAGE_DOWN) return outcome("page-down", true);
    const quiet = result.type === KEY_RESULT.SELECT_ALL ? "select-all" : "pass";
    if (isThirdLevelShift(ev, platform)) return outcome(quiet, false);
    if (!result.key) return outcome(quiet, result.cancel);
    // xterm.js leaves capital letters to its keypress handler (an input
    // method on macOS can otherwise send them lowercase); the character is
    // the same either way.
    if (ev.key && !ev.ctrlKey && !ev.altKey && !ev.metaKey && ev.key.length === 1) {
      const code = ev.key.charCodeAt(0);
      if (code >= 65 && code <= 90) return outcome("pass", result.cancel);
    }
    if (pending) {
      pending = false;
      return outcome("pass", result.cancel);
    }
    return outcome("send", true);
  }

  /**
   * The `key` message for a keydown `keydownAction` said to send, or null
   * when a browser value is past the format's 64-byte field bound (no
   * real key name is). `mac: true` says the viewer is a Mac by
   * `platformOf`'s list: some keys encode differently there in today's
   * xterm.js (Ctrl+Option with `_` or `@`, among others), so the
   * supervisor needs it.
   *
   * Any other viewer's message leaves `mac` out, which the format reads as
   * false. That is the canonical form `farhelm-proto` encodes and its golden
   * fixtures show (`key.json` has no `mac`; a `mac` written out as false is
   * only tolerated), and the unit tests hold this function to those
   * fixtures.
   */
  function keyMessage(ev, platform) {
    const key = typeof ev.key === "string" ? ev.key : "";
    const code = typeof ev.code === "string" ? ev.code : "";
    if (utf8Length(key) > MAX_KEY_FIELD_BYTES || utf8Length(code) > MAX_KEY_FIELD_BYTES) return null;
    const kc = Number.isInteger(ev.keyCode) && ev.keyCode >= 0 ? ev.keyCode : 0;
    return withMacFlag({ t: "key", key, code, kc, mods: modifiers(ev) }, platform);
  }

  /**
   * `message` (a `key` message the caller just built) with `mac: true` added
   * for a Mac viewer and left as it is otherwise (see `keyMessage`). Adds the
   * field in place and returns the same object.
   */
  function withMacFlag(message, platform) {
    if (platformOf(platform).isMac) message.mac = true;
    return message;
  }

  function utf8Length(text) {
    let bytes = 0;
    for (const ch of text) {
      const cp = ch.codePointAt(0);
      bytes += cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
    }
    return bytes;
  }

  // What a `text` message costs around its data once encoded
  // (`{"t":"text","data":""}`), with room to spare.
  const TEXT_ENVELOPE_BYTES = 64;
  const SHORT_ESCAPES = new Set(["\b", "\f", "\n", "\r", "\t", '"', "\\"]);

  /**
   * The bytes one character takes inside a JSON string once encoded:
   * JSON.stringify writes a control character as a six-byte `\u00XX`
   * escape (two bytes for the five with short escapes, and for `"` and
   * `\`), and a lone surrogate as a six-byte escape too.
   */
  function jsonLength(ch) {
    if (SHORT_ESCAPES.has(ch)) return 2;
    const cp = ch.codePointAt(0);
    if (cp < 0x20 || (cp >= 0xd800 && cp <= 0xdfff)) return 6;
    return utf8Length(ch);
  }

  /**
   * Split typed text into `text` messages on character boundaries, each
   * within both bounds the format sets: 16 KiB of UTF-8 data, and 64 KiB
   * for the encoded message. The second matters for control-heavy text,
   * whose escapes make the JSON up to six times the data. Ordinary typing
   * is one short message; this matters for a long input-method commit or
   * a filled-in field. Nothing is dropped: every character is in exactly
   * one message.
   */
  function textMessages(data) {
    if (typeof data !== "string" || data === "") return [];
    const messages = [];
    let part = "";
    let bytes = 0;
    let encoded = 0;
    for (const ch of data) {
      const size = utf8Length(ch);
      const cost = jsonLength(ch);
      if (part && (bytes + size > MAX_TEXT_BYTES || encoded + cost > MAX_INPUT_MESSAGE - TEXT_ENVELOPE_BYTES)) {
        messages.push({ t: "text", data: part });
        part = "";
        bytes = 0;
        encoded = 0;
      }
      part += ch;
      bytes += size;
      encoded += cost;
    }
    if (part) messages.push({ t: "text", data: part });
    return messages;
  }

  const MAX_PASTE_BYTES = 32 * 1024;

  /**
   * Split pasted text into `paste` messages (grid-terminal.js's `paste`,
   * which terminal.js's attachment interception also uses): parts of at most
   * 32 KiB of UTF-8 that also encode within the 64 KiB message bound, split
   * only at character boundaries and never between a CR and the LF after
   * it, so the supervisor's line-ending conversion sees the text it would
   * see whole. The last part says so. Nothing is dropped.
   */
  function pasteMessages(data) {
    if (typeof data !== "string" || data === "") return [];
    const chars = Array.from(data);
    const parts = [];
    let part = "";
    let bytes = 0;
    let encoded = 0;
    for (let i = 0; i < chars.length; i++) {
      // A CR and the LF after it travel together.
      const unit = chars[i] === "\r" && chars[i + 1] === "\n" ? "\r\n" : chars[i];
      if (unit === "\r\n") i++;
      const size = utf8Length(unit);
      const cost = Array.from(unit).reduce((sum, ch) => sum + jsonLength(ch), 0);
      if (part && (bytes + size > MAX_PASTE_BYTES || encoded + cost > MAX_INPUT_MESSAGE - TEXT_ENVELOPE_BYTES)) {
        parts.push(part);
        part = "";
        bytes = 0;
        encoded = 0;
      }
      part += unit;
      bytes += size;
      encoded += cost;
    }
    if (part) parts.push(part);
    return parts.map((text, index) => ({ t: "paste", data: text, last: index === parts.length - 1 }));
  }

  const utf8Encoder = typeof TextEncoder === "function" ? new TextEncoder() : null;

  /**
   * One input message as the bytes of one binary data frame, or null when
   * it would exceed the 64 KiB input bound. The builders in this file keep
   * every message they make under it (`textMessages` counts the encoded
   * size), so null means a caller built a message some other way; the
   * caller reports it rather than sending a frame the peer must refuse.
   */
  function encodeInput(message) {
    const bytes = utf8Encoder.encode(JSON.stringify(message));
    return bytes.byteLength > MAX_INPUT_MESSAGE ? null : bytes;
  }

  // ## Mouse

  /**
   * The cell under a pointer offset (pixels from the screen's top left),
   * rounded and clamped the way xterm.js's `getCoords` does for mouse
   * reports, so a press on a cell boundary or outside the grid lands on
   * the same cell it does today.
   */
  function mouseCell(offsetX, offsetY, cellWidth, cellHeight, cols, rows) {
    const col = Math.min(Math.max(Math.ceil(offsetX / cellWidth), 1), cols);
    const row = Math.min(Math.max(Math.ceil(offsetY / cellHeight), 1), rows);
    return { x: col - 1, y: row - 1 };
  }

  /** A press or release's button (0 left, 1 middle, 2 right), or null. */
  function pressButton(button) {
    return button === 0 || button === 1 || button === 2 ? button : null;
  }

  /** The held button for a move, from `buttons`; -1 when none is held. */
  function moveButton(buttons) {
    if (buttons & 1) return 0;
    if (buttons & 4) return 1;
    if (buttons & 2) return 2;
    return -1;
  }

  /**
   * The button a wheel step reports: 0 for up (toward the top of the
   * history, a negative `deltaY`) and 1 for down, as xterm.js numbers wheel
   * buttons. Only the sign matters; `createWheelAccumulator` decides whether
   * the event scrolls a line at all.
   */
  function wheelButton(ev) {
    return ev.deltaY < 0 ? 0 : 1;
  }

  /**
   * The `mouse` message for a pointer event over `cell` (as `mouseCell`
   * gives it): `kind` is "down", "up", "move" or "wheel", `button` what
   * `pressButton` or `moveButton` gave (`wheelButton` for a wheel step),
   * and the modifiers the event carries. Whether the event is sent at all
   * (the program's mode, move deduplication, a wheel step that scrolls no
   * line) is the caller's decision; this only shapes the message, here
   * rather than in grid-terminal.js so the unit tests can hold it to the
   * format's golden fixtures.
   */
  function mouseReport(kind, button, cell, ev) {
    return { t: "mouse", kind, button, x: cell.x, y: cell.y, mods: modifiers(ev) };
  }

  /** The `focus` message for focus gained (`true`) or lost. */
  function focusReport(focused) {
    return { t: "focus", in: !!focused };
  }

  /**
   * Whether the program's mouse mode (`state.mouse`) asks for this event:
   * presses and releases from "click" up, moves with a button held from
   * "drag" up, and every move under "motion". The supervisor still drops
   * what the program's exact mode does not report (X10 has no releases).
   */
  function modeReports(mode, kind, button) {
    if (mode === "off") return false;
    if (kind === "down" || kind === "up" || kind === "wheel") return true;
    if (kind === "move") return mode === "motion" || (mode === "drag" && button !== -1);
    return false;
  }

  /**
   * Tracks the last mouse message sent so a move equal to it is not sent
   * again: xterm.js's `CoreMouseService` keeps the last REPORTED event of
   * any kind (`_lastEvent`) and drops a move whose cell, button, action and
   * Shift, Alt and Ctrl match it (`_equalEvents`; Meta is not compared).
   * Because the action is compared, a move is only ever a repeat of a
   * move: the first move after a press, release or wheel step in the same
   * cell is sent. Callers pass only messages they are about to send.
   */
  function createMouseDeduper() {
    let last = null;
    return {
      shouldSend(message) {
        const same = last !== null && message.kind === "move" && last.kind === "move" && last.x === message.x &&
          last.y === message.y && last.button === message.button && (last.mods & 7) === (message.mods & 7);
        if (same) return false;
        last = message;
        return true;
      },
      reset() {
        last = null;
      },
    };
  }

  /**
   * xterm.js's `consumeWheelEvent`: how many lines one wheel event scrolls,
   * accumulating partial lines from pixel deltas across events, with its
   * default sensitivities (1, and 5 under Alt or Ctrl). Shift+wheel is
   * horizontal and scrolls nothing. Today's terminal sends one report (or
   * one arrow key) per event whose amount is not zero, so callers only
   * need the sign of `deltaY` once this says the event counts.
   */
  function createWheelAccumulator() {
    let partial = 0;
    return {
      consume(ev, cellHeight, rows) {
        if (ev.deltaY === 0 || ev.shiftKey || !(cellHeight > 0)) return 0;
        let amount = ev.altKey || ev.ctrlKey ? ev.deltaY * 5 : ev.deltaY;
        if (ev.deltaMode === 0) {
          amount /= cellHeight;
          if (Math.abs(ev.deltaY) < 50) amount *= 0.3;
          partial += amount;
          amount = Math.floor(Math.abs(partial)) * (partial > 0 ? 1 : -1);
          partial %= 1;
        } else if (ev.deltaMode === 2) {
          amount *= rows;
        }
        return amount;
      },
    };
  }

  /**
   * Whether a wheel event goes to the program as a `wheel` message: when
   * it reports the mouse, and on the alternate screen without mouse
   * reporting, where today's terminal sends arrow keys instead (the
   * supervisor encodes those from the same message). On the normal screen
   * without mouse reporting the wheel scrolls the client's own history.
   */
  function wheelGoesToProgram(state) {
    return state.mouse !== "off" || state.altScreen;
  }

  // ## Alt-click moves the cursor (xterm.js's `altClickMovesCursor`)
  //
  // Today's terminal moves a shell's cursor to an Alt-clicked (Option on a
  // Mac) cell by typing arrow keys: `moveToCellSequence` in the vendored
  // bundle decides how many, from the cursor, the screen and its soft wraps.
  // `moveToCellSequence` below is that function, kept branch for branch with
  // the original (including the argument it passes that looks like a slip:
  // the parity is the point), and the unit tests hold it to the bundle's own
  // function. Its sequences are made with normal cursor keys; the client
  // sends them as `key` messages (`arrowKeyMessages`), so the supervisor
  // encodes each with the program's cursor-key mode, as xterm.js would have.

  /** The arrow sequence for direction letter `dir` ("A" up, "B" down, "C" right, "D" left). */
  function arrowSequence(dir, applicationCursor) {
    return C0.ESC + (applicationCursor ? "O" : "[") + dir;
  }

  function repeatSequence(count, sequence) {
    let out = "";
    for (let i = 0; i < Math.floor(count); i++) out += sequence;
    return out;
  }

  /** How many rows above `row` it continues (xterm.js's `wrappedRowsForRow`). */
  function wrappedRowsForRow(row, bufferService) {
    let count = 0;
    let line = bufferService.buffer.lines.get(row);
    let isWrapped = line && line.isWrapped;
    while (isWrapped && row >= 0 && row < bufferService.rows) {
      count++;
      line = bufferService.buffer.lines.get(--row);
      isWrapped = line && line.isWrapped;
    }
    return count;
  }

  function verticalDirection(startY, targetY) {
    return startY > targetY ? "A" : "B";
  }

  function wrappedRowsCount(startY, targetY, bufferService) {
    let wrapped = 0;
    const startRow = startY - wrappedRowsForRow(startY, bufferService);
    const endRow = targetY - wrappedRowsForRow(targetY, bufferService);
    for (let i = 0; i < Math.abs(startRow - endRow); i++) {
      const direction = verticalDirection(startY, targetY) === "A" ? -1 : 1;
      const line = bufferService.buffer.lines.get(startRow + direction * i);
      if (line && line.isWrapped) wrapped++;
    }
    return wrapped;
  }

  function moveToRequestedRow(startY, targetY, bufferService, applicationCursor) {
    const startRow = startY - wrappedRowsForRow(startY, bufferService);
    const endRow = targetY - wrappedRowsForRow(targetY, bufferService);
    const rowsToMove = Math.abs(startRow - endRow) - wrappedRowsCount(startY, targetY, bufferService);
    return repeatSequence(rowsToMove, arrowSequence(verticalDirection(startY, targetY), applicationCursor));
  }

  /** The text between two cells, walking rows (xterm.js's `bufferLine`). */
  function bufferLine(startCol, startRow, endCol, endRow, forward, bufferService) {
    let currentCol = startCol;
    let currentRow = startRow;
    let text = "";
    while ((currentCol !== endCol || currentRow !== endRow) && currentRow >= 0 &&
      currentRow < bufferService.buffer.lines.length) {
      currentCol += forward ? 1 : -1;
      if (forward && currentCol > bufferService.cols - 1) {
        text += bufferService.buffer.translateBufferLineToString(currentRow, false, startCol, currentCol);
        currentCol = 0;
        startCol = 0;
        currentRow++;
      } else if (!forward && currentCol < 0) {
        text += bufferService.buffer.translateBufferLineToString(currentRow, false, 0, startCol + 1);
        currentCol = bufferService.cols - 1;
        startCol = currentCol;
        currentRow--;
      }
    }
    return text + bufferService.buffer.translateBufferLineToString(currentRow, false, startCol, currentCol);
  }

  function resetStartingRow(startX, startY, _targetX, targetY, bufferService, applicationCursor) {
    if (moveToRequestedRow(startY, targetY, bufferService, applicationCursor).length === 0) return "";
    return repeatSequence(
      bufferLine(startX, startY, startX, startY - wrappedRowsForRow(startY, bufferService), false, bufferService).length,
      arrowSequence("D", applicationCursor),
    );
  }

  function horizontalDirection(startX, startY, targetX, targetY, bufferService, applicationCursor) {
    // As the bundle has it: the requested-row test is given the target's
    // column where a row is expected.
    const startRow = moveToRequestedRow(targetX, targetY, bufferService, applicationCursor).length > 0
      ? targetY - wrappedRowsForRow(targetY, bufferService)
      : startY;
    return (startX < targetX && startRow <= targetY) || (startX >= targetX && startRow < targetY) ? "C" : "D";
  }

  function moveToRequestedCol(startX, startY, targetX, targetY, bufferService, applicationCursor) {
    const startRow = moveToRequestedRow(startY, targetY, bufferService, applicationCursor).length > 0
      ? targetY - wrappedRowsForRow(targetY, bufferService)
      : startY;
    const direction = horizontalDirection(startX, startY, targetX, targetY, bufferService, applicationCursor);
    return repeatSequence(
      bufferLine(startX, startRow, targetX, targetY, direction === "C", bufferService).length,
      arrowSequence(direction, applicationCursor),
    );
  }

  /**
   * The arrow keys that move the cursor from where it is to cell
   * (`targetX`, `targetY`) of the screen (0-based), as the vendored
   * xterm.js's `moveToCellSequence` makes them. `bufferService` is
   * `{cols, rows, buffer: {x, y, hasScrollback, lines: {length, get(row)},
   * translateBufferLineToString(row, trimRight, startCol, endCol)}}` with
   * screen rows as lines; `hasScrollback` is false on the alternate screen,
   * where the moves follow soft wraps and text, and true otherwise, where
   * they count cells.
   */
  function moveToCellSequence(targetX, targetY, bufferService, applicationCursor) {
    const startX = bufferService.buffer.x;
    const startY = bufferService.buffer.y;
    if (!bufferService.buffer.hasScrollback) {
      return resetStartingRow(startX, startY, targetX, targetY, bufferService, applicationCursor) +
        moveToRequestedRow(startY, targetY, bufferService, applicationCursor) +
        moveToRequestedCol(startX, startY, targetX, targetY, bufferService, applicationCursor);
    }
    if (startY === targetY) {
      return repeatSequence(Math.abs(startX - targetX), arrowSequence(startX > targetX ? "D" : "C", applicationCursor));
    }
    const direction = startY > targetY ? "D" : "C";
    const rowDifference = Math.abs(startY - targetY);
    const cellsToMove = (bufferService.cols - (startY > targetY ? targetX : startX)) + (rowDifference - 1) * bufferService.cols +
      1 + ((startY > targetY ? startX : targetX) - 1);
    return repeatSequence(cellsToMove, arrowSequence(direction, applicationCursor));
  }

  const ARROWS = Object.freeze({
    A: { key: "ArrowUp", kc: 38 },
    B: { key: "ArrowDown", kc: 40 },
    C: { key: "ArrowRight", kc: 39 },
    D: { key: "ArrowLeft", kc: 37 },
  });

  /**
   * One `key` message per arrow in a `moveToCellSequence` result (made with
   * normal cursor keys), for the supervisor to encode in the program's
   * cursor-key mode.
   */
  function arrowKeyMessages(sequence, platform) {
    const messages = [];
    for (let i = 0; i + 2 < sequence.length; i += 3) {
      const arrow = ARROWS[sequence[i + 2]];
      if (arrow) messages.push(withMacFlag({ t: "key", key: arrow.key, code: arrow.key, kc: arrow.kc, mods: 0 }, platform));
    }
    return messages;
  }

  const api = {
    MAX_INPUT_MESSAGE,
    MAX_KEY_FIELD_BYTES,
    MAX_PASTE_BYTES,
    MAX_TEXT_BYTES,
    MODS,
    arrowKeyMessages,
    createMouseDeduper,
    createWheelAccumulator,
    encodeInput,
    focusReport,
    isThirdLevelShift,
    keydownAction,
    keyMessage,
    modeReports,
    modifiers,
    mouseCell,
    mouseReport,
    moveToCellSequence,
    moveButton,
    pasteMessages,
    platformOf,
    pressButton,
    textMessages,
    wheelButton,
    wheelGoesToProgram,
    xtermKeyboardResult,
  };
  root.farhelmGridInput = api;
  if (typeof module === "object" && module.exports) module.exports = api;
})(typeof window === "object" ? window : globalThis);
