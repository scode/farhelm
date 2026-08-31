const test = require("node:test");
const assert = require("node:assert/strict");
const { authenticate } = require("../assets/desktop-auth.js");

// The native side of the IPC channel: scripted replies in, everything the
// script sends captured in `sent`.
function channel(replies) {
  const sent = [];
  return {
    sent,
    recv: async () => replies.shift(),
    send: (value) => sent.push(value),
  };
}

// The browser primitives the script is handed, with an in-memory
// localStorage whose contents (`values`) the tests read back directly.
// `options.throwRemove` makes removeItem throw, for the tests that pin
// cleanup as best-effort.
function platform(fetch, WebSocket, exchangeTimeoutMs, options = {}) {
  const values = new Map();
  return {
    fetch,
    WebSocket,
    AbortController,
    exchangeTimeoutMs,
    values,
    storage: {
      getItem: (key) => values.get(key) || null,
      setItem: (key, value) => values.set(key, value),
      removeItem: (key) => {
        if (options.throwRemove) throw new Error("removeItem refused");
        values.delete(key);
      },
    },
    setTimeout,
    clearTimeout,
  };
}

// The successful-validation socket most upgrade tests need: accepts the
// device credential by answering the event-socket handshake immediately.
class AcceptingSocket {
  constructor() {
    queueMicrotask(() => this.onmessage());
  }
  close() {}
}

// An upgraded webview still carries the localStorage keys the retired
// per-client preference persistence wrote. The preference lives in the helm
// now with no client-side copy wanted (SPEC.md, Session list), so a
// successful authentication is also the upgrade point that scrubs exactly
// those keys — and nothing else.
test("authentication scrubs the retired preference keys and keeps the rest", async () => {
  const ipc = channel([{
    base: "http://127.0.0.1:7433",
    token: "token",
    persisted: "persisted-device",
  }, { persisted: true }]);
  const browser = platform(async () => ({ ok: true, status: 204 }), AcceptingSocket);
  browser.values.set("farhelm.sort", "title");
  browser.values.set("farhelm.last-selected", JSON.stringify({ helm: "h", id: "old" }));
  browser.values.set("farhelm.unrelated", "kept");

  await authenticate(ipc, browser);

  assert.equal(browser.values.has("farhelm.sort"), false);
  assert.equal(browser.values.has("farhelm.last-selected"), false);
  assert.equal(browser.values.get("farhelm.unrelated"), "kept");
  assert.deepEqual(ipc.sent, [{ secret: "persisted-device" }, { ready: true }]);
});

// Cleanup is best-effort by contract: a storage that refuses removal must
// cost only the cleanup, never authentication readiness.
test("a storage that refuses removal still reaches ready", async () => {
  const ipc = channel([{
    base: "http://127.0.0.1:7433",
    token: "token",
    persisted: "persisted-device",
  }, { persisted: true }]);
  const browser = platform(
    async () => ({ ok: true, status: 204 }),
    AcceptingSocket,
    undefined,
    { throwRemove: true },
  );

  await authenticate(ipc, browser);

  assert.deepEqual(ipc.sent, [{ secret: "persisted-device" }, { ready: true }]);
});

// A valid device row followed by a transport-level socket failure is not an
// authentication rejection. Minting here would churn the bounded device table
// whenever the event feed is temporarily unavailable.
test("a WebSocket failure after successful validation never exchanges a token", async () => {
  const requests = [];
  const ipc = channel([{
    base: "http://127.0.0.1:7433",
    token: "bootstrap-token",
    persisted: "persisted-device",
  }]);
  class FailingSocket {
    constructor() {
      queueMicrotask(() => this.onerror());
    }
    close() {}
  }

  await authenticate(ipc, platform(async (url, options) => {
    requests.push([url, options]);
    return { ok: true, status: 204 };
  }, FailingSocket));

  assert.equal(requests.length, 1);
  assert.equal(requests[0][1].method, "GET");
  assert.deepEqual(ipc.sent, [{
    error: "webview event socket failed after device validation",
  }]);
});

// The bootstrap token is a file-backed rotation boundary. One rejected
// exchange asks Rust to re-read it; a second rejection remains a visible
// failure rather than becoming an unbounded retry loop.
test("an explicitly rejected exchange retries once with the re-read token", async () => {
  const tokens = [];
  const ipc = channel([
    { base: "http://127.0.0.1:7433", token: "stale-token", persisted: "" },
    { token: "current-token" },
    { persisted: true },
  ]);
  class AcceptedSocket {
    constructor() {
      queueMicrotask(() => this.onmessage());
    }
    close() {}
  }

  await authenticate(ipc, platform(async (_url, options) => {
    const token = JSON.parse(options.body).token;
    tokens.push(token);
    if (tokens.length === 1) return { ok: false, status: 401 };
    return {
      ok: true,
      status: 200,
      json: async () => ({ device_secret: "new-device" }),
    };
  }, AcceptedSocket));

  assert.deepEqual(tokens, ["stale-token", "current-token"]);
  assert.deepEqual(ipc.sent, [
    { retry_token: true },
    { secret: "new-device" },
    { ready: true },
  ]);
});

// The webview credential becomes durable in Rust first. Refusing that commit
// must leave browser storage untouched so the two stores cannot disagree after
// a crash or a failed atomic state-file replacement.
test("a rejected native persistence commit never reaches localStorage", async () => {
  const ipc = channel([
    { base: "http://127.0.0.1:7433", token: "token", persisted: "" },
    { persisted: false },
  ]);
  class AcceptedSocket {
    constructor() {
      queueMicrotask(() => this.onmessage());
    }
    close() {}
  }
  const browser = platform(async () => ({
    ok: true,
    status: 200,
    json: async () => ({ device_secret: "uncommitted-device" }),
  }), AcceptedSocket);

  await authenticate(ipc, browser);

  assert.equal(browser.values.get("farhelm.device-secret"), undefined);
  assert.deepEqual(ipc.sent, [
    { secret: "uncommitted-device" },
    { error: "native credential persistence failed" },
  ]);
});

// A responsive HTTP status with a stalled body is still a stalled exchange.
// The abort signal covers both fetch and response decoding under one deadline.
test("a stalled exchange body is aborted by the absolute deadline", async () => {
  const ipc = channel([
    { base: "http://127.0.0.1:7433", token: "token", persisted: "" },
  ]);
  class UnusedSocket {}
  const browser = platform(async (_url, options) => ({
    ok: true,
    status: 200,
    json: () => new Promise((_resolve, reject) => {
      options.signal.addEventListener("abort", () => reject(new Error("exchange aborted")));
    }),
  }), UnusedSocket, 10);

  await authenticate(ipc, browser);

  assert.deepEqual(ipc.sent, [{ error: "exchange aborted" }]);
});
