(function () {
  // Authenticate the desktop webview and maintain its browser-shaped copies.
  // The Dioxus eval and node tests execute this same state machine; injected
  // browser primitives keep the contract testable without pretending Node
  // has a webview or a WebSocket.
  function storePreference(storage, key, value) {
    try {
      if (value === undefined) return;
      if (value === null) storage.removeItem(key);
      else storage.setItem(key, value);
    } catch (_) {
      // Native remains authoritative. A full/disabled localStorage can leave
      // this page-side copy stale, but must not block readiness or the current
      // interaction.
    }
  }

  function applyPreferences(storage, preferences) {
    if (!preferences) return;
    const selection = preferences.remembered_selection;
    storePreference(
      storage,
      "farhelm.last-selected",
      selection === undefined || selection === null ? selection : JSON.stringify(selection),
    );
    storePreference(storage, "farhelm.sort", preferences.list_sort);
  }

  async function report(channel, storage) {
    const update = await channel.recv();
    applyPreferences(storage, update);
    channel.send(update);
  }

  async function authenticate(channel, platform) {
    const bootstrap = await channel.recv();
    try {
      async function accepted(secret) {
        if (!secret) return false;
        const wsBase = bootstrap.base.replace(/^http/, "ws");
        return await new Promise(function (resolve) {
          let settled = false;
          const finish = function (accepted) {
            if (settled) return;
            settled = true;
            platform.clearTimeout(deadline);
            socket.close();
            resolve(accepted);
          };
          const socket = new platform.WebSocket(
            `${wsBase}/api/events`,
            ["farhelm", `farhelm-device-${secret}`],
          );
          const deadline = platform.setTimeout(function () { finish(false); }, 5000);
          socket.onmessage = function () { finish(true); };
          socket.onerror = function () { finish(false); };
          socket.onclose = function () { finish(false); };
        });
      }

      let secret = bootstrap.persisted
        || platform.storage.getItem("farhelm.device-secret")
        || "";
      let authenticated = false;
      if (secret) {
        const validation = await platform.fetch(`${bootstrap.base}/api/auth/device`, {
          method: "GET",
          headers: { "Authorization": `Bearer ${secret}` },
          cache: "no-store",
        });
        if (validation.ok) {
          if (!(await accepted(secret))) {
            throw new Error("webview event socket failed after device validation");
          }
          authenticated = true;
        } else if (validation.status !== 401) {
          throw new Error(`webview device validation failed with ${validation.status}`);
        }
      }
      if (!authenticated) {
        async function exchange(token) {
          const controller = new platform.AbortController();
          const deadline = platform.setTimeout(function () {
            controller.abort();
          }, platform.exchangeTimeoutMs || 5000);
          try {
            const response = await platform.fetch(`${bootstrap.base}/api/auth/token`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ token }),
              signal: controller.signal,
            });
            const body = response.ok ? await response.json() : null;
            return { response, body };
          } finally {
            platform.clearTimeout(deadline);
          }
        }
        let result = await exchange(bootstrap.token);
        let response = result.response;
        if (response.status === 401) {
          channel.send({ retry_token: true });
          const retry = await channel.recv();
          result = await exchange(retry.token);
          response = result.response;
        }
        if (!response.ok) {
          throw new Error(`webview device exchange failed with ${response.status}`);
        }
        secret = result.body.device_secret;
        if (!(await accepted(secret))) {
          throw new Error("webview event socket failed after device exchange");
        }
      }
      channel.send({ secret });
      const commit = await channel.recv();
      if (!commit.persisted) {
        throw new Error("native credential persistence failed");
      }
      try {
        platform.storage.setItem("farhelm.device-secret", secret);
      } catch (_) {
        // Native committed the credential first and can seed the next launch.
        // Browser storage is a reusable cache, not an authentication commit.
      }
      // Native sends BOTH keys on first bootstrap, including nulls for
      // absent state, so its state file is authoritative over stale webview
      // storage. Reauthentication omits the seed (`null`) and leaves choices
      // made since launch untouched. The storage writes are attempted before
      // `ready`, the message Rust gates the whole component tree on.
      applyPreferences(platform.storage, bootstrap.preferences);
      channel.send({ ready: true });
    } catch (error) {
      channel.send({ error: String(error && error.message ? error.message : error) });
    }
  }

  if (typeof module !== "undefined" && module.exports) {
    module.exports = { applyPreferences, authenticate, report };
  } else {
    window.__farhelmDesktopPreferences = { report };
    return authenticate(
      {
        recv: function () { return dioxus.recv(); },
        send: function (value) { dioxus.send(value); },
      },
      {
        fetch: window.fetch.bind(window),
        AbortController: window.AbortController,
        WebSocket: window.WebSocket,
        storage: window.localStorage,
        setTimeout: window.setTimeout.bind(window),
        clearTimeout: window.clearTimeout.bind(window),
      },
    );
  }
})();
