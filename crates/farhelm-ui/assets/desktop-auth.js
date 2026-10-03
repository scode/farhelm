(function () {
  // Authenticate the desktop webview: install the credential native hands
  // over in page memory where the page's own consumers read it, and prove
  // the helm accepts it. The Dioxus eval and node tests execute this same
  // state machine; injected browser primitives keep the contract testable
  // without pretending Node has a webview or a WebSocket.
  //
  // The credential is an in-memory one the desktop app's embedded helm
  // minted for this launch (farhelm_helm::EmbeddedReady). Token rotation and
  // the helm's cap on remembered browser credentials never revoke it, so
  // there is nothing to mint, refresh, or persist here: a refusal means
  // something is broken and is reported as an error. The web token (the
  // helm's root credential) never reaches this script at all (SPEC.md
  // "Client hardening").
  //
  // Authentication is ALL this script does, apart from scrubbing keys older
  // builds left in localStorage (a stored webview credential and retired
  // preference keys).
  async function authenticate(channel, platform) {
    const bootstrap = await channel.recv();
    try {
      async function accepted(secret) {
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

      const secret = bootstrap.secret;
      if (!secret) {
        throw new Error("native handed over no webview credential");
      }
      // Keep the launch credential in the page's memory. WebKit persists
      // localStorage to disk, which would make an embedded helm's
      // per-launch secret survive the process that minted it. terminal.js
      // and events.js consult this global before their browser storage
      // fallback, so desktop requests never need to write the secret.
      platform.page.__farhelmWebviewDeviceSecret = secret;
      // Older builds wrote this key. Remove it on every credentialed
      // authentication attempt (a no-op once it is gone), but do not let a storage
      // implementation that refuses cleanup strand an otherwise valid launch.
      try {
        platform.storage.removeItem("farhelm.device-secret");
      } catch (_error) {
        // Legacy data is only hygiene; validation below remains authoritative.
      }

      // Bounded, because nothing else times this step out (the native side
      // waits on this script without a deadline): a helm that accepts the
      // connection and never answers would otherwise leave the window on
      // "Starting Farhelm…" forever with no error.
      const controller = new platform.AbortController();
      const deadline = platform.setTimeout(function () {
        controller.abort();
      }, platform.validationTimeoutMs || 5000);
      let validation;
      try {
        validation = await platform.fetch(`${bootstrap.base}/api/auth/device`, {
          method: "GET",
          headers: { "Authorization": `Bearer ${secret}` },
          cache: "no-store",
          signal: controller.signal,
        });
      } catch (error) {
        if (controller.signal.aborted) {
          throw new Error("webview device validation timed out");
        }
        throw error;
      } finally {
        platform.clearTimeout(deadline);
      }
      if (!validation.ok) {
        throw new Error(`webview device validation failed with ${validation.status}`);
      }
      if (!(await accepted(secret))) {
        throw new Error("webview event socket failed after device validation");
      }
      // Scrub the keys the retired per-client preference persistence used.
      // An upgraded webview keeps whatever an old build stored there, and
      // the preference now lives in the helm with no client-side copy
      // wanted (SPEC.md, Session list). Best-effort, one key at a time:
      // cleanup must never block readiness, and a failure on one key must
      // not strand the other.
      for (const retired of ["farhelm.sort", "farhelm.last-selected"]) {
        try {
          platform.storage.removeItem(retired);
        } catch (_) {
          // A blocked or broken storage costs only the cleanup.
        }
      }
      // `ready` is the message Rust gates the whole component tree on.
      channel.send({ ready: true });
    } catch (error) {
      channel.send({ error: String(error && error.message ? error.message : error) });
    }
  }

  if (typeof module !== "undefined" && module.exports) {
    module.exports = { authenticate };
  } else {
    return authenticate(
      {
        recv: function () { return dioxus.recv(); },
        send: function (value) { dioxus.send(value); },
      },
      {
        fetch: window.fetch.bind(window),
        AbortController: window.AbortController,
        WebSocket: window.WebSocket,
        page: window,
        storage: (function () {
          try {
            return window.localStorage;
          } catch (_error) {
            return null;
          }
        })(),
        setTimeout: window.setTimeout.bind(window),
        clearTimeout: window.clearTimeout.bind(window),
      },
    );
  }
})();
