// The clipboard bridge is intentionally asynchronous, but terminal parsing
// must never wait on it. These tests pin the bounded queue and latest-value
// semantics against the exact plain-script asset shipped to the browser.
const test = require("node:test");
const assert = require("node:assert/strict");
const { createClipboardWriter } = require("../assets/clipboard-writer.js");

test("a never-settling write retains only one pending value", () => {
  const submitted = [];
  const write = createClipboardWriter((text) => {
    submitted.push(text);
    return new Promise(() => {});
  });

  write("first");
  write("old pending");
  write("latest");

  assert.deepEqual(submitted, ["first"]);
});

test("the latest pending value follows a resolved write", async () => {
  const submitted = [];
  let resolve;
  const write = createClipboardWriter((text) => {
    submitted.push(text);
    return new Promise((done) => {
      resolve = done;
    });
  });

  write("first");
  write("stale");
  write("latest");
  resolve();
  await Promise.resolve();

  assert.deepEqual(submitted, ["first", "latest"]);
});

test("the latest pending value follows a rejected write", async () => {
  const submitted = [];
  let reject;
  const write = createClipboardWriter((text) => {
    submitted.push(text);
    return new Promise((_, fail) => {
      reject = fail;
    });
  });

  write("first");
  write("stale");
  write("latest");
  reject(new Error("clipboard refused"));
  await Promise.resolve();

  assert.deepEqual(submitted, ["first", "latest"]);
});

test("a synchronous write does not block the next value", () => {
  const submitted = [];
  const write = createClipboardWriter((text) => {
    submitted.push(text);
  });

  write("first");
  write("second");

  assert.deepEqual(submitted, ["first", "second"]);
});
