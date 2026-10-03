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

// The browser primitives the script is handed, with an in-memory page global
// and localStorage whose contents (`values`) the tests read back directly.
// `options.storage: null` simulates unavailable storage, and
// `options.throwRemove` makes removeItem throw so the tests can pin cleanup
// as best-effort.
function platform(fetch, WebSocket, options = {}) {
  const values = new Map();
  const removals = [];
  const writes = [];
  const page = {};
  const storage = options.storage === null ? null : {
    getItem: (key) => values.get(key) || null,
    setItem: (key, value) => {
      writes.push([key, value]);
      values.set(key, value);
    },
    removeItem: (key) => {
      removals.push(key);
      if (options.throwRemove) throw new Error("removeItem refused");
      values.delete(key);
    },
  };
  return {
    fetch,
    WebSocket,
    page,
    AbortController,
    values,
    removals,
    writes,
    storage,
    setTimeout,
    clearTimeout,
  };
}

// The successful-validation socket most tests need: accepts the device
// credential by answering the event-socket handshake immediately.
class AcceptingSocket {
  constructor() {
    queueMicrotask(() => this.onmessage());
  }
  close() {}
}

// A socket the tests that must stop before validation can assert was never
// opened.
class UnusedSocket {
  constructor() {
    throw new Error("the event socket must not be opened");
  }
}

const BOOTSTRAP = { base: "http://127.0.0.1:7433", secret: "launch-device" };

// The ordinary launch: the handed-over credential lands in page memory where
// the page's terminals, attachment uploads and event feed read it, the helm
// accepts it, and the script reports ready. Nothing is minted and nothing is
// sent back but `ready`: the credential is the embedded helm's in-memory one,
// so there is no exchange to run and no disk write to make.
test("a launch keeps the handed-over credential in page memory", async () => {
  const requests = [];
  const ipc = channel([BOOTSTRAP]);
  const browser = platform(async (url, options) => {
    requests.push([url, options]);
    return { ok: true, status: 204 };
  }, AcceptingSocket);

  await authenticate(ipc, browser);

  assert.equal(browser.page.__farhelmWebviewDeviceSecret, "launch-device");
  assert.deepEqual(browser.writes, [], "desktop authentication never persists the credential");
  assert.deepEqual(requests.map(([url]) => url), ["http://127.0.0.1:7433/api/auth/device"]);
  assert.equal(requests[0][1].headers.Authorization, "Bearer launch-device");
  assert.deepEqual(ipc.sent, [{ ready: true }]);
});

// A legacy secret is removed during authentication. A storage implementation
// that refuses that cleanup cannot make a valid in-memory credential fail.
test("legacy desktop storage cleanup is best effort", async () => {
  for (const stale of [null, "previous-launch-device"]) {
    const ipc = channel([BOOTSTRAP]);
    const browser = platform(async () => ({ ok: true, status: 204 }), AcceptingSocket, {
      throwRemove: true,
    });
    if (stale) browser.values.set("farhelm.device-secret", stale);

    await authenticate(ipc, browser);

    assert.deepEqual(ipc.sent, [{ ready: true }], `stale=${stale}`);
    assert.ok(browser.removals.includes("farhelm.device-secret"), `stale=${stale}`);
    assert.equal(browser.page.__farhelmWebviewDeviceSecret, "launch-device");
    assert.equal(
      browser.values.get("farhelm.device-secret"),
      stale || undefined,
      "the test premise: refused cleanup leaves the legacy value in place",
    );
  }
});

// Storage is only a migration cleanup path now. A webview that disables or
// rejects localStorage must still be able to authenticate from page memory.
test("unavailable localStorage does not block authentication", async () => {
  const ipc = channel([BOOTSTRAP]);
  const browser = platform(async () => ({ ok: true, status: 204 }), AcceptingSocket, {
    storage: null,
  });

  await authenticate(ipc, browser);

  assert.equal(browser.page.__farhelmWebviewDeviceSecret, "launch-device");
  assert.deepEqual(ipc.sent, [{ ready: true }]);
});

// Successful cleanup removes the old key without touching unrelated page
// storage. The page global remains the sole desktop credential source.
test("authentication removes the legacy desktop secret", async () => {
  const ipc = channel([BOOTSTRAP]);
  const browser = platform(async () => ({ ok: true, status: 204 }), AcceptingSocket);
  browser.values.set("farhelm.device-secret", "previous-launch-device");
  browser.values.set("farhelm.unrelated", "kept");

  await authenticate(ipc, browser);

  assert.equal(browser.values.has("farhelm.device-secret"), false);
  assert.equal(browser.values.get("farhelm.unrelated"), "kept");
  assert.equal(browser.page.__farhelmWebviewDeviceSecret, "launch-device");
  assert.deepEqual(ipc.sent, [{ ready: true }]);
});

// The helm refusing the credential is reported, never repaired. There is no
// token exchange to fall back to (the web token never reaches the page), and
// a refusal of an in-memory credential means something is broken, which the
// failure page is for.
test("a refused credential is reported, and the page never exchanges a token", async () => {
  const requests = [];
  const ipc = channel([BOOTSTRAP]);

  await authenticate(ipc, platform(async (url) => {
    requests.push(url);
    return { ok: false, status: 401 };
  }, UnusedSocket));

  assert.ok(
    requests.every((url) => !url.endsWith("/api/auth/token")),
    `the page must not exchange a token itself: ${requests}`,
  );
  assert.deepEqual(ipc.sent, [{ error: "webview device validation failed with 401" }]);
});

// A missing credential from native is a broken hand-off, reported before
// anything touches storage.
test("a hand-off without a credential is reported", async () => {
  const ipc = channel([{ base: "http://127.0.0.1:7433", secret: "" }]);
  const browser = platform(async () => ({ ok: true, status: 204 }), UnusedSocket);

  await authenticate(ipc, browser);

  assert.equal(browser.page.__farhelmWebviewDeviceSecret, undefined);
  assert.deepEqual(ipc.sent, [{ error: "native handed over no webview credential" }]);
});

// A valid credential followed by a socket failure is reported as such: the
// page's event feed cannot run, so the window must not open as if it could.
test("a WebSocket failure after successful validation is reported", async () => {
  const ipc = channel([BOOTSTRAP]);
  class FailingSocket {
    constructor() {
      queueMicrotask(() => this.onerror());
    }
    close() {}
  }

  await authenticate(ipc, platform(async () => ({ ok: true, status: 204 }), FailingSocket));

  assert.deepEqual(ipc.sent, [{
    error: "webview event socket failed after device validation",
  }]);
});

// An upgraded webview still carries the localStorage keys the retired
// per-client preference persistence wrote. The preference lives in the helm
// now with no client-side copy wanted (SPEC.md, Session list), so a
// successful authentication also scrubs those preference keys and leaves
// unrelated keys alone.
test("authentication scrubs the retired preference keys and keeps the rest", async () => {
  const ipc = channel([BOOTSTRAP]);
  const browser = platform(async () => ({ ok: true, status: 204 }), AcceptingSocket);
  browser.values.set("farhelm.sort", "title");
  browser.values.set("farhelm.last-selected", JSON.stringify({ helm: "h", id: "old" }));
  browser.values.set("farhelm.unrelated", "kept");

  await authenticate(ipc, browser);

  assert.equal(browser.values.has("farhelm.sort"), false);
  assert.equal(browser.values.has("farhelm.last-selected"), false);
  assert.equal(browser.values.get("farhelm.unrelated"), "kept");
  assert.deepEqual(ipc.sent, [{ ready: true }]);
});

// Cleanup is best-effort by contract: a storage that refuses removal must
// cost only the cleanup, never authentication readiness.
test("a storage that refuses removal still reaches ready", async () => {
  const ipc = channel([BOOTSTRAP]);
  const browser = platform(
    async () => ({ ok: true, status: 204 }),
    AcceptingSocket,
    { throwRemove: true },
  );

  await authenticate(ipc, browser);

  assert.deepEqual(ipc.sent, [{ ready: true }]);
});

// The validation request runs on every launch and nothing else times it
// out, so a helm that accepts the request and never answers used to leave
// the window on "Starting Farhelm…" forever. It is bounded, and the timeout
// reaches native as an ordinary, visible error.
test("a validation request that never answers times out as an error", async () => {
  const ipc = channel([BOOTSTRAP]);
  const browser = platform((_url, options) => new Promise((_resolve, reject) => {
    options.signal.addEventListener("abort", () => reject(new Error("aborted")));
  }), UnusedSocket);
  browser.validationTimeoutMs = 10;

  await authenticate(ipc, browser);

  assert.deepEqual(ipc.sent, [{ error: "webview device validation timed out" }]);
});
