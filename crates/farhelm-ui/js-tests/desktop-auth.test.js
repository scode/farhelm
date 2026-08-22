const test = require("node:test");
const assert = require("node:assert/strict");
const { authenticate, report } = require("../assets/desktop-auth.js");

function channel(replies, trace = []) {
  const sent = [];
  return {
    sent,
    recv: async () => replies.shift(),
    send: (value) => {
      sent.push(value);
      trace.push({ kind: "message", value });
    },
  };
}

function platform(fetch, WebSocket, exchangeTimeoutMs, options = {}) {
  const values = new Map();
  const trace = options.trace || [];
  return {
    fetch,
    WebSocket,
    AbortController,
    exchangeTimeoutMs,
    values,
    storage: {
      getItem: (key) => values.get(key) || null,
      setItem: (key, value) => {
        if (options.throwSet) throw new Error("setItem refused");
        values.set(key, value);
        trace.push({ kind: "set", key, value });
      },
      removeItem: (key) => {
        if (options.throwRemove) throw new Error("removeItem refused");
        values.delete(key);
        trace.push({ kind: "remove", key });
      },
    },
    setTimeout,
    clearTimeout,
  };
}

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

test("bootstrap seeds preferences before reporting ready", async () => {
  const trace = [];
  const ipc = channel([{
    base: "http://127.0.0.1:7433",
    token: "token",
    persisted: "persisted-device",
    preferences: {
      remembered_selection: { helm: "helm-a", id: "session-old" },
      list_sort: "title",
    },
  }, { persisted: true }], trace);
  class AcceptedSocket {
    constructor() {
      queueMicrotask(() => this.onmessage());
    }
    close() {}
  }
  const browser = platform(
    async () => ({ ok: true, status: 204 }),
    AcceptedSocket,
    undefined,
    { trace },
  );

  await authenticate(ipc, browser);

  assert.equal(
    browser.values.get("farhelm.last-selected"),
    JSON.stringify({ helm: "helm-a", id: "session-old" }),
  );
  assert.equal(browser.values.get("farhelm.sort"), "title");
  assert.deepEqual(ipc.sent, [
    { secret: "persisted-device" },
    { ready: true },
  ]);
  const ready = trace.findIndex((entry) => entry.kind === "message" && entry.value.ready);
  const selection = trace.findIndex((entry) => entry.key === "farhelm.last-selected");
  const sort = trace.findIndex((entry) => entry.key === "farhelm.sort");
  assert.ok(selection >= 0 && selection < ready, "selection must be stored before ready");
  assert.ok(sort >= 0 && sort < ready, "sort must be stored before ready");
});

test("preference reports use the browser keys and echo the native payload", async () => {
  const update = {
    remembered_selection: { helm: "helm-a", id: "session-picked" },
  };
  const ipc = channel([update]);
  const browser = platform(() => {}, class UnusedSocket {});

  await report(ipc, browser.storage);

  assert.equal(
    browser.values.get("farhelm.last-selected"),
    JSON.stringify(update.remembered_selection),
  );
  assert.deepEqual(ipc.sent, [update]);
});

test("null bootstrap preference fields clear stale browser copies", async () => {
  const ipc = channel([{
    base: "http://127.0.0.1:7433",
    token: "token",
    persisted: "persisted-device",
    preferences: { remembered_selection: null, list_sort: null },
  }, { persisted: true }]);
  class AcceptedSocket {
    constructor() { queueMicrotask(() => this.onmessage()); }
    close() {}
  }
  const browser = platform(async () => ({ ok: true, status: 204 }), AcceptedSocket);
  browser.values.set("farhelm.last-selected", "stale-selection");
  browser.values.set("farhelm.sort", "created");

  await authenticate(ipc, browser);

  assert.equal(browser.values.has("farhelm.last-selected"), false);
  assert.equal(browser.values.has("farhelm.sort"), false);
  assert.deepEqual(ipc.sent.at(-1), { ready: true });
});

test("reauthentication without a preference seed preserves post-launch choices", async () => {
  const ipc = channel([{
    base: "http://127.0.0.1:7433",
    token: "token",
    persisted: "persisted-device",
    preferences: null,
  }, { persisted: true }]);
  class AcceptedSocket {
    constructor() { queueMicrotask(() => this.onmessage()); }
    close() {}
  }
  const browser = platform(async () => ({ ok: true, status: 204 }), AcceptedSocket);
  browser.values.set("farhelm.last-selected", "post-launch-selection");
  browser.values.set("farhelm.sort", "title");

  await authenticate(ipc, browser);

  assert.equal(browser.values.get("farhelm.last-selected"), "post-launch-selection");
  assert.equal(browser.values.get("farhelm.sort"), "title");
  assert.deepEqual(ipc.sent.at(-1), { ready: true });
});

test("storage write failures do not block authentication readiness", async () => {
  const ipc = channel([{
    base: "http://127.0.0.1:7433",
    token: "token",
    persisted: "persisted-device",
    preferences: {
      remembered_selection: { helm: "helm-a", id: "session-a" },
      list_sort: "title",
    },
  }, { persisted: true }]);
  class AcceptedSocket {
    constructor() { queueMicrotask(() => this.onmessage()); }
    close() {}
  }
  const browser = platform(
    async () => ({ ok: true, status: 204 }),
    AcceptedSocket,
    undefined,
    { throwSet: true },
  );

  await authenticate(ipc, browser);

  assert.deepEqual(ipc.sent, [{ secret: "persisted-device" }, { ready: true }]);
});

test("storage removal failures do not block authentication readiness", async () => {
  const ipc = channel([{
    base: "http://127.0.0.1:7433",
    token: "token",
    persisted: "persisted-device",
    preferences: { remembered_selection: null, list_sort: null },
  }, { persisted: true }]);
  class AcceptedSocket {
    constructor() { queueMicrotask(() => this.onmessage()); }
    close() {}
  }
  const browser = platform(
    async () => ({ ok: true, status: 204 }),
    AcceptedSocket,
    undefined,
    { throwRemove: true },
  );

  await authenticate(ipc, browser);

  assert.deepEqual(ipc.sent, [{ secret: "persisted-device" }, { ready: true }]);
});

test("preference report acknowledgement survives storage failures", async () => {
  const update = { list_sort: "title" };
  const ipc = channel([update]);
  const browser = platform(
    () => {},
    class UnusedSocket {},
    undefined,
    { throwSet: true },
  );

  await report(ipc, browser.storage);

  assert.deepEqual(ipc.sent, [update]);
});

test("sparse preference reports preserve the other browser value", async () => {
  const browser = platform(() => {}, class UnusedSocket {});
  browser.values.set("farhelm.last-selected", JSON.stringify({ helm: "helm-a", id: "old" }));
  browser.values.set("farhelm.sort", "created");

  await report(channel([{
    remembered_selection: { helm: "helm-a", id: "new" },
  }]), browser.storage);
  assert.equal(browser.values.get("farhelm.sort"), "created");

  await report(channel([{ list_sort: "title" }]), browser.storage);
  assert.equal(
    browser.values.get("farhelm.last-selected"),
    JSON.stringify({ helm: "helm-a", id: "new" }),
  );
});
