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

  /** Platform names as xterm.js's `Browser` module reads them. */
  function platformOf(platform) {
    const name = typeof platform === "string" ? platform : "";
    return {
      // terminal.js and the page's shortcuts read Mac-ness the same way.
      isMac: /Mac/.test(name),
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
   * real key name is).
   */
  function keyMessage(ev) {
    const key = typeof ev.key === "string" ? ev.key : "";
    const code = typeof ev.code === "string" ? ev.code : "";
    if (utf8Length(key) > MAX_KEY_FIELD_BYTES || utf8Length(code) > MAX_KEY_FIELD_BYTES) return null;
    const kc = Number.isInteger(ev.keyCode) && ev.keyCode >= 0 ? ev.keyCode : 0;
    return { t: "key", key, code, kc, mods: modifiers(ev) };
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
   * Split pasted text into `paste` messages (the paste itself is Tier 2;
   * this is the format's splitting rule, ready for it): parts of at most
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
   * Tracks the last mouse message sent so a move within the same cell, with
   * the same button and modifiers, is not sent again (xterm.js's
   * `_equalEvents` check, which compares Shift, Alt and Ctrl only).
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

  const api = {
    MAX_INPUT_MESSAGE,
    MAX_TEXT_BYTES,
    MODS,
    createMouseDeduper,
    createWheelAccumulator,
    encodeInput,
    isThirdLevelShift,
    keydownAction,
    keyMessage,
    modeReports,
    modifiers,
    mouseCell,
    moveButton,
    pasteMessages,
    pressButton,
    textMessages,
    wheelGoesToProgram,
    xtermKeyboardResult,
  };
  root.farhelmGridInput = api;
  if (typeof module === "object" && module.exports) module.exports = api;
})(typeof window === "object" ? window : globalThis);
