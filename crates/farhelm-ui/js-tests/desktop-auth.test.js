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

// The web token never reaches the page. A page with no stored secret, or
// one the helm refuses, asks native to mint a device secret and only ever
// sees that; it never calls the token exchange itself. Native re-reads a
// rotated token on its side (auth.rs `mint_webview_secret`).
test("a page without a usable secret asks native to mint one", async () => {
  for (const persisted of ["", "refused-device"]) {
    const requests = [];
    const ipc = channel([
      { base: "http://127.0.0.1:7433", persisted },
      { secret: "new-device" },
      { persisted: true },
    ]);
    class AcceptedSocket {
      constructor() {
        queueMicrotask(() => this.onmessage());
      }
      close() {}
    }

    await authenticate(ipc, platform(async (url) => {
      requests.push(url);
      return { ok: false, status: 401 };
    }, AcceptedSocket));

    assert.ok(
      requests.every((url) => !url.endsWith("/api/auth/token")),
      `the page must not exchange a token itself: ${requests}`,
    );
    assert.deepEqual(ipc.sent, [
      { need_secret: true },
      { secret: "new-device" },
      { ready: true },
    ]);
  }
});

// A native mint failure (a refused or unreachable helm) reaches the gate as
// the page's ordinary, visible error.
test("a native mint failure is reported as the authentication error", async () => {
  const ipc = channel([
    { base: "http://127.0.0.1:7433", persisted: "" },
    { error: "webview device exchange failed with 503 Service Unavailable" },
  ]);
  class UnusedSocket {}

  await authenticate(ipc, platform(async () => ({ ok: true, status: 204 }), UnusedSocket));

  assert.deepEqual(ipc.sent, [
    { need_secret: true },
    { error: "webview device exchange failed with 503 Service Unavailable" },
  ]);
});

// The webview credential becomes durable in Rust first. Refusing that commit
// must leave browser storage untouched so the two stores cannot disagree after
// a crash or a failed atomic state-file replacement.
test("a rejected native persistence commit never reaches localStorage", async () => {
  const ipc = channel([
    { base: "http://127.0.0.1:7433", persisted: "" },
    { secret: "uncommitted-device" },
    { persisted: false },
  ]);
  class AcceptedSocket {
    constructor() {
      queueMicrotask(() => this.onmessage());
    }
    close() {}
  }
  const browser = platform(async () => ({ ok: true, status: 204 }), AcceptedSocket);

  await authenticate(ipc, browser);

  assert.equal(browser.values.get("farhelm.device-secret"), undefined);
  assert.deepEqual(ipc.sent, [
    { need_secret: true },
    { secret: "uncommitted-device" },
    { error: "native credential persistence failed" },
  ]);
});

// The saved-credential check runs on nearly every launch and nothing else
// times it out, so a helm that accepts the request and never answers used to
// leave the window on "Starting Farhelm…" forever. It is bounded like the
// exchange, and the timeout reaches native as an ordinary, visible error.
test("a validation request that never answers times out as an error", async () => {
  const ipc = channel([
    { base: "http://127.0.0.1:7433", persisted: "persisted-device" },
  ]);
  class UnusedSocket {}
  const browser = platform((_url, options) => new Promise((_resolve, reject) => {
    options.signal.addEventListener("abort", () => reject(new Error("aborted")));
  }), UnusedSocket);
  browser.validationTimeoutMs = 10;

  await authenticate(ipc, browser);

  assert.deepEqual(ipc.sent, [{ error: "webview device validation timed out" }]);
});

