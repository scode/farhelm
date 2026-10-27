import type { Page } from "@playwright/test";

export type TunnelPreviewState = "one" | "one-list" | "many" | "many-list" | "closing" | "error" | "error-list";

/** Mirror native-only tunnel markup for CSS checks in the real browser frame.
 * This is the Steel plan's explicit visual fallback, matching tunnels.rs and
 * ActionIcon. Invented mappings prove layout and colors; the display-only buttons do
 * not simulate Dioxus callbacks, API requests, consent, or native ownership.
 * Native Rust tests and code review supply that separate evidence.
 */
export async function mountNativeTunnelPreview(page: Page, state: TunnelPreviewState, hostKind: "long" | "short" = "long") {
  const host = hostKind === "short" ? "devbox" : "gpu-1-with-a-deliberately-long-host-name.example.test";
  await page.evaluate(({ state, host }) => {
    document.querySelectorAll("[data-tunnel-preview]").forEach((element) => element.remove());
    const topbar = document.querySelector(".session-topbar");
    const actions = topbar?.querySelector(".titlebar-actions");
    if (!topbar || !actions) throw new Error("the real toolbar must precede the native preview");
    const icon = (glyph: "tunnel" | "chevron-down" | "close" | "warning") => {
      const paths = {
        tunnel: "M5.5 8.5l3-3M5 5l1.8-1.8a2.8 2.8 0 0 1 4 4L9 9M5 5 3.2 6.8a2.8 2.8 0 0 0 4 4L9 9",
        "chevron-down": "M3 5l4 4 4-4",
        close: "M3 3l8 8M11 3l-8 8",
        warning: "M7 1.5 13 12H1zM7 5v3.2M7 10h.01",
      };
      return `<svg class="action-glyph" data-glyph="${glyph}" width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><path d="${paths[glyph]}"></path></svg>`;
    };
    // All values here are closed fixture data, never a peer/API response.
    const mapping = (local: number, remote: number, tooltip = false) =>
      `<span class="session-tunnel-mapping"${tooltip ? ` data-tooltip=":${local} → ${host}:${remote}"` : ""}><span>:${local} → </span><span class="session-tunnel-host peer-value" dir="ltr">${host}</span><span>:${remote}</span></span>`;
    const holder = document.createElement("div");
    holder.dataset.tunnelPreview = state;
    const errorCopy = `${icon("warning")}<div><p class="peer-value" dir="ltr">:57779 → ${host}:44227: The tunnel could not be closed</p></div><button type="button" class="btn">Dismiss</button>`;
    if (state === "error") {
      holder.className = "session-tunnel-error";
      holder.setAttribute("role", "alert");
      holder.dataset.linkCover = "true";
      holder.innerHTML = errorCopy;
    } else {
      holder.className = "session-tunnel-pill";
      holder.dataset.rowMenuKey = "header:tunnels:preview";
      const multiple = state === "many" || state === "many-list" || state === "closing";
      const open = state === "one-list" || state === "many-list" || state === "closing" || state === "error-list";
      const busy = state === "closing";
      holder.dataset.terminalKeyboardOwner = String(open);
      const help = multiple ? "2 open tunnels from this Mac" : `Open tunnels from this Mac: :57779 → ${host}:44227`;
      holder.innerHTML = `<button type="button" class="btn session-tunnel-toggle bar-menu-toggle" aria-label="${help}" aria-expanded="${open}" aria-controls="session-tunnel-panel" data-tooltip="${help}">${icon("tunnel")}${multiple ? "<span class=\"session-tunnel-count\">2 tunnels</span>" : mapping(57779, 44227)}${icon("chevron-down")}</button>`;
      if (open) {
        const rows = (multiple ? [[57779, 44227], [65535, 65534]] : [[57779, 44227]]).map(([local, remote]) =>
          `<li class="session-tunnel-row">${mapping(local, remote, true)}<button type="button" class="btn session-tunnel-close" data-tunnel-id="preview-${local}" aria-label="Close tunnel from local port ${local} to remote port ${remote}" data-tooltip="Close tunnel" aria-disabled="${busy}">${icon("close")}</button></li>`
        ).join("");
        const error = state === "error-list" ? `<div class="session-tunnel-error session-tunnel-inline-error" role="alert" data-link-cover="true">${errorCopy}</div>` : "";
        holder.insertAdjacentHTML("beforeend", `<div id="session-tunnel-panel" class="host-row-menu-panel bar-menu-panel session-tunnel-panel" role="group" aria-label="Open tunnels from this Mac" data-link-cover="true"><div class="menu-header session-row-menu-header"><div class="menu-title session-row-menu-title">Tunnels from this Mac</div><div class="menu-summary session-row-menu-summary">Local listeners stay open while the host is offline</div></div>${error}<ul class="session-tunnel-list">${rows}</ul><div class="menu-separator" role="separator"></div><button type="button" class="btn menu-item session-tunnel-close-all" aria-disabled="${busy}">${icon("close")}${busy ? "Closing…" : "Close all"}</button></div>`);
      }
    }
    topbar.insertBefore(holder, actions);
  }, { state, host });
}
