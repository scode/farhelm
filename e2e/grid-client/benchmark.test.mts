/** Pure harness checks: malformed framing or ambiguous ownership must fail before a measurement can count. */
import assert from "node:assert/strict";
import test from "node:test";
import { contentProcess, FrameDecoder, parseMemory, parseStat } from "./benchmark.mts";

/** A private envelope can split at any network boundary without changing the grid payload bytes. */
test("frames survive split prefixes and payloads and do not retain a larger chunk", () => {
  const wire = Uint8Array.of(0, 0, 0, 3, 5, 6, 7, 0, 0, 0, 1, 8);
  for (let split = 0; split <= wire.length; split++) {
    const decoder = new FrameDecoder();
    const frames = [...decoder.push(wire.subarray(0, split)), ...decoder.push(wire.subarray(split))];
    decoder.finish();
    assert.deepEqual(frames.map((frame) => [...frame]), [[5, 6, 7], [8]]);
    assert.equal(decoder.frames, 2);
    assert.equal(decoder.bytes, 4);
    for (const frame of frames) assert.equal(frame.buffer.byteLength, frame.byteLength);
  }
});

/** EOF is not success when an incomplete prefix or payload could conceal lost output. */
test("truncated and oversized benchmark envelopes fail", () => {
  for (const bytes of [Uint8Array.of(0), Uint8Array.of(0, 0, 0, 3, 1)]) {
    const decoder = new FrameDecoder();
    decoder.push(bytes);
    assert.throws(() => decoder.finish(), /truncated/);
  }
  for (const bytes of [Uint8Array.of(0, 0, 0, 0), Uint8Array.of(0, 16, 0, 1)]) {
    assert.throws(() => new FrameDecoder().push(bytes), /length/);
  }
});

/** Kernel comm is parenthesized text, not whitespace-delimited fields. */
test("process identity parses awkward comm without losing start ticks", () => {
  const fields = Array(20).fill("0");
  fields[0] = "S";
  fields[1] = "41";
  fields[19] = "99999999999999999";
  assert.deepEqual(parseStat(`42 (a worker (with) spaces) ${fields.join(" ")}`), {
    pid: 42,
    parent: 41,
    start: "99999999999999999",
  });
  assert.throws(() => parseStat("42 (worker) S 41"), /malformed/);
});

/** Missing kernel readings cannot become plausible zero-valued memory samples. */
test("memory readings preserve bytes and require both RSS and HWM", () => {
  assert.deepEqual(parseMemory("Name:\tworker\nVmRSS:\t123 kB\nVmHWM:\t456 kB\n"), {
    rss: 123 * 1024,
    hwm: 456 * 1024,
  });
  assert.throws(() => parseMemory("VmRSS: 123 kB\n"), /VmHWM/);
});

/** Chromium helpers and a second renderer must never silently enter the page's denominator. */
test("content attribution requires exactly one engine-specific owned process", () => {
  const root = { pid: 1, parent: 0, start: "1", args: ["browser"] };
  const renderer = { pid: 2, parent: 1, start: "2", args: ["chrome", "--type=renderer"] };
  const gpu = { pid: 3, parent: 1, start: "3", args: ["chrome", "--type=gpu-process"] };
  const web = { pid: 4, parent: 1, start: "4", args: ["/fictional/browser/WPEWebProcess", "11"] };
  assert.equal(contentProcess("chromium", [root, renderer, gpu]), renderer);
  const titled = { ...renderer, args: ["/fictional/chrome --type=renderer --headless --no-sandbox"] };
  assert.equal(contentProcess("chromium", [root, titled, gpu]), titled);
  assert.throws(
    () => contentProcess("chromium", [{ ...renderer, args: ["chrome --note=--type=renderer"] }]),
    /found 0/,
  );
  assert.throws(
    () => contentProcess("chromium", [{ ...renderer, args: ["chrome --type=renderer-helper"] }]),
    /found 0/,
  );
  assert.equal(contentProcess("webkit", [root, web]), web);
  assert.throws(() => contentProcess("chromium", [root, gpu]), /found 0/);
  assert.throws(() => contentProcess("chromium", [renderer, { ...renderer, pid: 5 }]), /found 2/);
  assert.throws(() => contentProcess("webkit", [renderer]), /found 0/);
});
