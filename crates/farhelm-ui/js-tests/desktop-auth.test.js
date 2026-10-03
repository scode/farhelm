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
// cleanup as best-effort; `options.throwSet` makes setItem throw, and
// `options.dropSet` makes it silently keep the old value.
function platform(fetch, WebSocket, options = {}) {
  const values = new Map();
  return {
    fetch,
    WebSocket,
    AbortController,
    values,
    storage: {
      getItem: (key) => values.get(key) || null,
      setItem: (key, value) => {
        if (options.throwSet) throw new Error("QuotaExceededError");
        if (options.dropSet) return;
        values.set(key, value);
      },
      removeItem: (key) => {
        if (options.throwRemove) throw new Error("removeItem refused");
        values.delete(key);
      },
    },
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

// The ordinary launch: the handed-over credential lands where the page's
// terminals, uploads and event feed read it, the helm accepts it, and the
// script reports ready. Nothing is minted and nothing is sent back but
// `ready`: the credential is the embedded helm's in-memory one, so there is
// no exchange to run and no commit to make.
test("a launch stores the handed-over credential, validates it, and reports ready", async () => {
  const requests = [];
  const ipc = channel([BOOTSTRAP]);
  const browser = platform(async (url, options) => {
    requests.push([url, options]);
    return { ok: true, status: 204 };
  }, AcceptingSocket);

  await authenticate(ipc, browser);

  assert.equal(browser.values.get("farhelm.device-secret"), "launch-device");
  assert.deepEqual(requests.map(([url]) => url), ["http://127.0.0.1:7433/api/auth/device"]);
  assert.equal(requests[0][1].headers.Authorization, "Bearer launch-device");
  assert.deepEqual(ipc.sent, [{ ready: true }]);
});

// A localStorage write that fails is an authentication failure, not a
// shrug. The page's consumers read their credential from storage, so a
// window opened after a failed write would run its terminals and event feed
// on no credential, or on a stale one from an earlier launch that the helm
// no longer accepts, while reporting itself ready. This used to be swallowed.
test("a throwing localStorage write fails authentication, even over a stale value", async () => {
  for (const stale of [null, "previous-launch-device"]) {
    const ipc = channel([BOOTSTRAP]);
    const browser = platform(async () => ({ ok: true, status: 204 }), UnusedSocket, {
      throwSet: true,
    });
    if (stale) browser.values.set("farhelm.device-secret", stale);

    await authenticate(ipc, browser);

    assert.equal(ipc.sent.length, 1, `stale=${stale}`);
    assert.match(ipc.sent[0].error, /^storing the webview credential failed: QuotaExceededError/);
    assert.equal(
      browser.values.get("farhelm.device-secret"),
      stale || undefined,
      "the test premise: storage still holds whatever was there before",
    );
  }
});

// A write that throws nothing but does not take is the same failure: what
// matters is what the consumers will read back, not whether setItem
// complained.
test("a localStorage write that does not read back fails authentication", async () => {
  const ipc = channel([BOOTSTRAP]);
  const browser = platform(async () => ({ ok: true, status: 204 }), UnusedSocket, {
    dropSet: true,
  });
  browser.values.set("farhelm.device-secret", "previous-launch-device");

  await authenticate(ipc, browser);

  assert.deepEqual(ipc.sent, [{
    error: "storing the webview credential failed: it did not read back",
  }]);
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

  assert.equal(browser.values.has("farhelm.device-secret"), false);
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
// successful authentication is also the upgrade point that scrubs exactly
// those keys — and nothing else.
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
