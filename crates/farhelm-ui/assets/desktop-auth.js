(function () {
  // Authenticate the desktop webview: install the credential native hands
  // over where the page's own consumers read it, and prove the helm accepts
  // it. The Dioxus eval and node tests execute this same state machine;
  // injected browser primitives keep the contract testable without
  // pretending Node has a webview or a WebSocket.
  //
  // The credential is an in-memory one the desktop app's embedded helm
  // minted for this launch (farhelm_helm::EmbeddedReady). Token rotation and
  // the helm's cap on remembered browser credentials never revoke it, so
  // there is nothing to mint, refresh, or persist here: a refusal means
  // something is broken and is reported as an error. The web token (the
  // helm's root credential) never reaches this script at all (SPEC.md
  // "Client hardening").
  //
  // Authentication is ALL this script does, apart from scrubbing keys a
  // retired feature left in localStorage.
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
      // Stored FIRST, and a write that does not take is fatal. The
      // terminals, uploads and event feed read their credential from
      // localStorage, not from this script, so a window that opened after a
      // failed write would run them on a missing secret, or on a stale one
      // an earlier launch left behind that the helm no longer accepts.
      try {
        platform.storage.setItem("farhelm.device-secret", secret);
      } catch (error) {
        throw new Error(`storing the webview credential failed: ${error && error.message ? error.message : error}`);
      }
      if (platform.storage.getItem("farhelm.device-secret") !== secret) {
        throw new Error("storing the webview credential failed: it did not read back");
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
        storage: window.localStorage,
        setTimeout: window.setTimeout.bind(window),
        clearTimeout: window.clearTimeout.bind(window),
      },
    );
  }
})();
