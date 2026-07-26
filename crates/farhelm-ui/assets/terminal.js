// The xterm.js island. Dioxus owns everything around the terminal; this
// file owns the terminal's content path: WebSocket bytes go straight into
// term.write() and keystrokes go straight back, bypassing the reactive
// layer entirely (SPEC_impl.md: the bypass is load-bearing — PTY-rate
// output through a vdom would be a performance disaster).
//
// Loaded as plain scripts (xterm.js, addon-fit.js, then this) — no
// bundler, no CDN: the UI must be fully self-contained.

(function () {
  "use strict";

  // Watermark backpressure seam (SPEC_impl.md): term.write() buffers
  // asynchronously up to a hard ~50MB cap, then silently discards, so
  // track unwritten bytes via write callbacks. M1 only observes
  // (interactive agent output never approaches these rates); the
  // pause/resume message to the supervisor hangs off this counter when
  // it lands.
  const HIGH_WATER = 4 * 1024 * 1024;

  window.farhelmTerm = {
    /**
     * Mount a terminal into #elementId, attached to the helm terminal
     * WebSocket at wsPath (e.g. /api/sessions/<id>/term).
     *
     * baseUrl is the helm's absolute HTTP origin in both builds — the
     * page's own origin for the web build, FARHELM_URL for the desktop
     * webview (whose origin is not the helm). An empty string falls back
     * to the current page's host, which only happens if origin lookup
     * failed.
     */
    mount(elementId, wsPath, baseUrl) {
      // Re-renders may call mount again; one terminal per page in M1.
      if (window.__farhelmMounted) return null;
      window.__farhelmMounted = true;
      const el = document.getElementById(elementId);
      const term = new Terminal({
        scrollback: 12000,
        fontSize: 14,
        cursorBlink: true,
      });
      const fit = new FitAddon.FitAddon();
      term.loadAddon(fit);
      term.open(el);
      fit.fit();

      const base = baseUrl
        ? baseUrl.replace(/^http/, "ws")
        : (location.protocol === "https:" ? "wss://" : "ws://") + location.host;
      const ws = new WebSocket(
        `${base}${wsPath}?cols=${term.cols}&rows=${term.rows}`,
      );
      ws.binaryType = "arraybuffer";

      let pendingWrite = 0;
      ws.onmessage = (ev) => {
        if (typeof ev.data === "string") {
          // Text frames are control JSON from the helm; today that is
          // only the detach notice (SPEC.md: takeover must be visible).
          const msg = JSON.parse(ev.data);
          if (msg.type === "detached") {
            showBanner(`Detached: ${msg.reason}`);
          }
          return;
        }
        const bytes = new Uint8Array(ev.data);
        pendingWrite += bytes.length;
        term.write(bytes, () => {
          pendingWrite -= bytes.length;
        });
        if (pendingWrite > HIGH_WATER) {
          // Backpressure seam: replace with a pause message when the
          // end-to-end plumbing lands.
          console.warn("farhelm: terminal write backlog", pendingWrite);
        }
      };
      // A detach notice is immediately followed by the server closing
      // the socket; the close handler must not clobber the more specific
      // banner (the takeover message is the one SPEC.md requires the
      // user to see).
      let bannered = false;
      ws.onclose = () => showBanner("Connection closed");
      ws.onerror = () => showBanner("Connection error");

      const enc = new TextEncoder();
      term.onData((d) => {
        if (ws.readyState === WebSocket.OPEN) ws.send(enc.encode(d));
      });
      // onBinary carries mouse reports and other non-UTF8 input as a
      // binary string; encode byte-for-byte.
      term.onBinary((d) => {
        if (ws.readyState !== WebSocket.OPEN) return;
        const bytes = new Uint8Array(d.length);
        for (let i = 0; i < d.length; i++) bytes[i] = d.charCodeAt(i) & 0xff;
        ws.send(bytes);
      });

      const sendResize = () => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(
            JSON.stringify({ type: "resize", cols: term.cols, rows: term.rows }),
          );
        }
      };
      term.onResize(sendResize);
      // A resize between socket construction and open would otherwise be
      // dropped forever, leaving the pane sized to the stale dimensions
      // in the connect URL.
      ws.addEventListener("open", sendResize);
      window.addEventListener("resize", () => fit.fit());

      // Sticky by design, enforced HERE rather than at each call site:
      // the first banner wins for the life of the socket, so the
      // specific reason (a takeover) is never overwritten by the generic
      // close or error that follows it a moment later. Callers must not
      // need to remember to check the flag.
      function showBanner(text) {
        if (bannered) return;
        bannered = true;
        const banner = document.getElementById("term-banner");
        if (banner) {
          banner.textContent = text;
          banner.style.display = "block";
        }
      }

      term.focus();
      // Test hooks: tests wait on the flag instead of sleeping, read
      // terminal content through the buffer API — the DOM renderer only
      // materializes viewport rows, so DOM text misses scrollback — and
      // reach the raw socket to exercise message-size limits that
      // keyboard-driven input cannot produce.
      window.__farhelmTerm = term;
      window.__farhelmWs = ws;
      window.__farhelmTermReady = true;
      return { term, ws };
    },
  };
})();
