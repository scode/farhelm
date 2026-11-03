// Unit tests of program-input.ts's decoding, which the shortcut and dialog
// specs trust to say exactly what a program read. Run with
// `node --test e2e/tests/helpers/program-input.test.ts` (Node strips the
// types); the Playwright configs never pick this file up.
import assert from "node:assert/strict";
import { test } from "node:test";
import { type InputFrame, programInput, recordInput } from "./program-input.ts";

const grid = { kind: "grid" as const };
const bytes = { kind: "bytes" as const };
const frame = (socket: InputFrame["socket"], value: unknown): InputFrame => ({
  socket,
  payload: Buffer.from(typeof value === "string" ? value : JSON.stringify(value)),
});
/** A key message as the grid client sends it, with the key's real numeric code. */
const key = (k: string, kc: number, mods = 0) => ({ t: "key", key: k, code: "", kc, mods, mac: false });

/** Why: a byte stream's frames are the program's bytes; reading one that looks
 * like a grid message as such would hide leaked input on old terminals. */
test("a byte stream's frames read as their bytes, JSON-shaped or not", () => {
  assert.equal(programInput([frame(bytes, '{"t":"text","data":""}')]), '{"t":"text","data":""}');
  assert.equal(programInput([frame(bytes, "probe"), frame(bytes, "\r")]), "probe\r");
  // A socket that never got a stream notice speaks bytes, as terminal.js reads it.
  assert.equal(programInput([frame({ kind: null }, '{"t":"text","data":"x"}')]), '{"t":"text","data":"x"}');
});

/** Why: Shift turns Enter and Tab into other bytes (ESC CR, CSI Z), so a
 * shifted named key must never read as the plain one; and the supervisor
 * picks a named key's or a Ctrl letter's bytes by its numeric code. */
test("grid keys read as the supervisor encodes them, by their numeric codes", () => {
  assert.equal(programInput([frame(grid, key("a", 65)), frame(grid, key("A", 65, 1)), frame(grid, key("Enter", 13))]), "aA\r");
  assert.equal(programInput([frame(grid, key("7", 55)), frame(grid, key("/", 191))]), "7/");
  assert.equal(programInput([frame(grid, key("Enter", 13, 1))]), "\x1b\r");
  assert.equal(programInput([frame(grid, key("Tab", 9))]), "\t");
  assert.equal(programInput([frame(grid, key("Tab", 9, 1))]), "\x1b[Z");
  assert.equal(programInput([frame(grid, key("Backspace", 8)), frame(grid, key("Escape", 27))]), "\x7f\x1b");
  assert.equal(programInput([frame(grid, key("k", 75, 4))]), "\x0b");
  assert.equal(programInput([frame(grid, { t: "text", data: "typed" })]), "typed");
});

/** Why: a message whose code the supervisor would read differently, or not
 * at all, must not read as the bytes its name suggests. */
test("a key whose numeric code is missing or disagrees with its name reads as a token", () => {
  // kc 0: the supervisor sends nothing for a key it cannot place.
  assert.equal(programInput([frame(grid, key("Enter", 0))]), "‹key 0:0:Enter›");
  assert.equal(programInput([frame(grid, key("a", 0))]), "‹key 0:0:a›");
  assert.equal(programInput([frame(grid, key("k", 0, 4))]), "‹key 4:0:k›");
  // A code for another key: the supervisor would write that key's bytes.
  assert.equal(programInput([frame(grid, key("k", 74, 4))]), "‹key 4:74:k›");
  assert.equal(programInput([frame(grid, key("Enter", 9))]), "‹key 0:9:Enter›");
  // F5's code types nothing printable, whatever `key` says.
  assert.equal(programInput([frame(grid, key("x", 116))]), "‹key 0:116:x›");
});

/** Why: the supervisor refuses a key message missing a field the format
 * requires, so the program reads nothing; the observer must not report the
 * bytes the key's name and code would otherwise give. */
test("a key message missing its code, or with a field of the wrong type, reads as a token", () => {
  const { code: _code, ...noCode } = key("k", 75, 4);
  assert.equal(programInput([frame(grid, noCode)]), "‹key malformed›");
  assert.equal(programInput([frame(grid, { ...key("k", 75, 4), code: 7 })]), "‹key malformed›");
  assert.equal(programInput([frame(grid, { ...key("k", 75, 4), mac: "no" })]), "‹key malformed›");
  // The same Ctrl+K, well formed, is its control character.
  assert.equal(programInput([frame(grid, { ...key("k", 75, 4), code: "KeyK" })]), "\x0b");
});

/** Why: anything the decoder cannot place must still fail an exact
 * comparison rather than read as nothing or as a plain key. */
test("grid messages it cannot place read as tokens", () => {
  assert.equal(programInput([frame(grid, key("Enter", 13, 4))]), "‹key 4:13:Enter›");
  assert.equal(programInput([frame(grid, key("a", 65, 2))]), "‹key 2:65:a›");
  assert.equal(programInput([frame(grid, key("constructor", 0))]), "‹key 0:0:constructor›");
  assert.equal(programInput([frame(grid, { t: "paste", data: "x", last: true })]), "‹paste›");
  assert.equal(programInput([frame(grid, "not json")]), "‹unreadable›");
});

/** Why: the kind is the attachment's, learned from the helm's notice on that
 * socket, and frames keep the kind of the socket they went out on. */
test("recordInput tags each sent frame with its socket's notice", () => {
  const listeners: Record<string, ((frame: { payload: string | Buffer }) => void)[]> = {};
  const socket = {
    on(event: string, listener: (frame: { payload: string | Buffer }) => void) {
      (listeners[event] ??= []).push(listener);
    },
  };
  const frames: InputFrame[] = [];
  recordInput(socket, frames);
  listeners.framereceived.forEach((l) => l({ payload: '{"type":"stream","kind":"grid"}' }));
  listeners.framesent.forEach((l) => l({ payload: Buffer.from(JSON.stringify(key("x", 88))) }));
  listeners.framesent.forEach((l) => l({ payload: '{"type":"resize"}' }));
  assert.equal(frames.length, 1);
  assert.equal(programInput(frames), "x");
});
