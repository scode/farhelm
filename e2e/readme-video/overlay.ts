/**
 * On-screen annotation for the demo video (docs/readme-video/SPEC.md,
 * "Annotations"): a visible pointer, callouts with arrows, highlights,
 * captions, and title cards, drawn by the page itself so they are part of
 * every recorded frame.
 *
 * Annotations are anchored to elements, never to coordinates. A callout is
 * given a locator; the page re-measures that element on every animation
 * frame and redraws the arrow to wherever it is. That is what lets the
 * video's wording stay fixed while the UI under it changes: a moved status
 * dot moves its arrow with it.
 *
 * The overlay lives in a shadow root on a fixed, full-viewport host with
 * `pointer-events: none`, which buys three guarantees the choreography
 * relies on: scripted clicks go through to the app as if the overlay were
 * not there, nothing about the app's layout shifts, and the app's CSS and
 * the overlay's CSS cannot restyle each other.
 *
 * Headless Chromium draws no mouse pointer, so a click would otherwise be
 * invisible in the recording. {@link Director.click} glides the overlay's
 * pointer to the target, pulses it, and then clicks the same point.
 */
import type { Locator, Page } from "@playwright/test";

/** Where a callout's text box sits relative to its target. `auto` picks the side with the most room. */
export type Side = "auto" | "left" | "right" | "top" | "bottom";

/**
 * Where a callout's box goes: a side of the target, then shifted by `dx`/`dy`
 * pixels. The shift is how a capture spec moves a box off something the
 * viewer should still see (the arrow keeps pointing at the target); the box
 * is still kept on screen afterwards.
 */
export interface Placement {
  side?: Side;
  dx?: number;
  dy?: number;
}

/** A drawn annotation that can be faded out again. */
export interface Annotation {
  dismiss(): Promise<void>;
}

/** How long fades take, in the page and in the waits that follow them. */
const FADE_MS = 350;

/**
 * The in-page half. Serialised by Playwright and evaluated in the page, so
 * it must be self-contained: no imports, no references to module scope.
 * Exposes `window.__farhelmDemo`; idempotent so it can run both as an init
 * script and directly against an already-loaded page.
 */
function installOverlay(fadeMs: number): void {
  const w = window as any;
  if (w.__farhelmDemo) return;

  const css = `
    :host { all: initial; }
    * { box-sizing: border-box; }
    .layer { position: fixed; inset: 0; font-family: "JetBrains Mono", ui-monospace, monospace; }
    .fade { opacity: 0; transition: opacity ${fadeMs}ms ease; }
    .fade.shown { opacity: 1; }
    svg.arrows { position: fixed; inset: 0; width: 100vw; height: 100vh; overflow: visible; }
    svg.arrows path { fill: none; stroke: #ff5fa2; stroke-width: 4; stroke-linecap: round; }
    .callout {
      position: fixed; max-width: 420px; padding: 14px 18px; border-radius: 10px;
      background: rgba(18, 12, 20, 0.94); border: 2px solid #ff5fa2; color: #fff;
      font-size: 20px; line-height: 1.4; box-shadow: 0 10px 30px rgba(0, 0, 0, 0.55);
    }
    .ring {
      position: fixed; border: 4px solid #ff5fa2; border-radius: 12px;
      box-shadow: 0 0 22px rgba(255, 95, 162, 0.65);
    }
    .ring.spotlight { box-shadow: 0 0 22px rgba(255, 95, 162, 0.65), 0 0 0 200vmax rgba(0, 0, 0, 0.55); }
    .caption {
      position: fixed; left: 50%; bottom: 48px; transform: translateX(-50%); max-width: 70vw;
      padding: 14px 26px; border-radius: 12px; background: rgba(10, 10, 14, 0.9);
      border: 2px solid #ff5fa2; color: #fff; font-size: 24px; text-align: center;
    }
    .card {
      position: fixed; inset: 0; display: flex; flex-direction: column; align-items: center;
      justify-content: center; gap: 22px; background: rgba(8, 10, 14, 0.8);
      backdrop-filter: blur(10px); color: #fff; text-align: center;
    }
    .card h1 { margin: 0; font-size: 64px; font-weight: 700; letter-spacing: -0.02em; }
    .card h1::after { content: ""; display: inline-block; width: 0.55em; height: 0.95em; margin-left: 0.12em;
      vertical-align: -0.1em; background: #ff5fa2; }
    .card p { margin: 0; font-size: 26px; color: #c9d1dc; max-width: 60vw; }
    .cursor {
      position: fixed; left: 0; top: 0; width: 28px; height: 28px; opacity: 0;
      transition-property: transform, opacity; transition-timing-function: cubic-bezier(0.45, 0, 0.25, 1);
      filter: drop-shadow(0 2px 3px rgba(0, 0, 0, 0.6));
    }
    .anchor { position: fixed; opacity: 0; }
    .pulse {
      position: fixed; width: 44px; height: 44px; margin: -22px 0 0 -22px; border-radius: 50%;
      border: 3px solid #ff5fa2; animation: pulse 520ms ease-out forwards;
    }
    @keyframes pulse { from { transform: scale(0.3); opacity: 1; } to { transform: scale(1.4); opacity: 0; } }
  `;

  const start = () => {
    const host = document.createElement("farhelm-demo-overlay");
    host.style.cssText = "position:fixed;inset:0;z-index:2147483647;pointer-events:none;";
    const root = host.attachShadow({ mode: "open" });
    root.innerHTML = `<style>${css}</style>
      <div class="layer">
        <svg class="arrows" xmlns="http://www.w3.org/2000/svg"></svg>
        <svg class="cursor" viewBox="0 0 28 28" xmlns="http://www.w3.org/2000/svg">
          <path d="M3 2 L3 22 L8.5 17 L12.5 26 L16 24.5 L12 15.8 L19.5 15.8 Z"
                fill="#fff" stroke="#111" stroke-width="1.6" stroke-linejoin="round"/>
        </svg>
      </div>`;
    document.documentElement.appendChild(host);
    const layer = root.querySelector(".layer") as HTMLElement;
    const arrows = root.querySelector("svg.arrows") as SVGSVGElement;
    const cursor = root.querySelector(".cursor") as SVGSVGElement;

    // Everything anchored to an element re-measures it every frame, so the
    // annotation follows layout changes, scrolling, and resizes.
    const trackers = new Map<number, () => void>();
    const nodes = new Map<number, Element[]>();
    let nextId = 1;
    const tick = () => {
      for (const update of trackers.values()) update();
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);

    const show = (id: number, elements: Element[], update?: () => void) => {
      nodes.set(id, elements);
      if (update) {
        update();
        trackers.set(id, update);
      }
      // Two frames: one to lay out at opacity 0, one to start the fade.
      requestAnimationFrame(() => requestAnimationFrame(() => elements.forEach((el) => el.classList.add("shown"))));
      return id;
    };

    /**
     * Place a box of size bw x bh beside rect r on `side`, kept on screen.
     * Returns the box origin and the arrow's two ends: from the middle of
     * the box edge facing the target to the nearest point on the target's
     * facing edge.
     */
    const place = (r: DOMRect, bw: number, bh: number, wanted: string, dx: number, dy: number) => {
      const W = innerWidth, H = innerHeight, gap = 80, margin = 16;
      const room: Record<string, number> = {
        right: W - r.right - bw, left: r.left - bw, bottom: H - r.bottom - bh, top: r.top - bh,
      };
      const side = wanted === "auto" ? Object.entries(room).sort((a, b) => b[1] - a[1])[0][0] : wanted;
      const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), Math.max(lo, hi));
      let x: number, y: number;
      if (side === "right" || side === "left") {
        x = side === "right" ? r.right + gap : r.left - gap - bw;
        y = r.top + r.height / 2 - bh / 2;
      } else {
        x = r.left + r.width / 2 - bw / 2;
        y = side === "bottom" ? r.bottom + gap : r.top - gap - bh;
      }
      x = clamp(x + dx, margin, W - bw - margin);
      y = clamp(y + dy, margin, H - bh - margin);
      const inset = 6;
      let from: [number, number], to: [number, number];
      if (side === "right") {
        from = [x, y + bh / 2];
        to = [r.right + inset, clamp(from[1], r.top + 4, r.bottom - 4)];
      } else if (side === "left") {
        from = [x + bw, y + bh / 2];
        to = [r.left - inset, clamp(from[1], r.top + 4, r.bottom - 4)];
      } else if (side === "bottom") {
        from = [x + bw / 2, y];
        to = [clamp(from[0], r.left + 4, r.right - 4), r.bottom + inset];
      } else {
        from = [x + bw / 2, y + bh];
        to = [clamp(from[0], r.left + 4, r.right - 4), r.top - inset];
      }
      return { x, y, from, to };
    };

    /** A gently curved arrow with an open head, as one path. */
    const arrowPath = (from: [number, number], to: [number, number]) => {
      const [x1, y1] = from, [x2, y2] = to;
      const dx = x2 - x1, dy = y2 - y1, len = Math.hypot(dx, dy) || 1;
      const cx = (x1 + x2) / 2 - (dy / len) * len * 0.18, cy = (y1 + y2) / 2 + (dx / len) * len * 0.18;
      // The head points along the curve's final tangent (control point to tip).
      const tx = x2 - cx, ty = y2 - cy, tl = Math.hypot(tx, ty) || 1, ux = tx / tl, uy = ty / tl, head = 16;
      const hx1 = x2 - ux * head - uy * head * 0.6, hy1 = y2 - uy * head + ux * head * 0.6;
      const hx2 = x2 - ux * head + uy * head * 0.6, hy2 = y2 - uy * head - ux * head * 0.6;
      return `M${x1},${y1} Q${cx},${cy} ${x2},${y2} M${hx1},${hy1} L${x2},${y2} L${hx2},${hy2}`;
    };

    w.__farhelmDemo = {
      callout(target: Element, text: string, side: string, dx: number, dy: number): number {
        const id = nextId++;
        const box = document.createElement("div");
        box.className = "callout fade";
        box.textContent = text;
        layer.appendChild(box);
        const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
        path.classList.add("fade");
        arrows.appendChild(path);
        return show(id, [box, path], () => {
          const spot = place(target.getBoundingClientRect(), box.offsetWidth, box.offsetHeight, side, dx, dy);
          box.style.left = `${spot.x}px`;
          box.style.top = `${spot.y}px`;
          path.setAttribute("d", arrowPath(spot.from, spot.to));
        });
      },
      highlight(target: Element, spotlight: boolean, pad: number): number {
        const id = nextId++;
        const ring = document.createElement("div");
        ring.className = `ring fade${spotlight ? " spotlight" : ""}`;
        layer.appendChild(ring);
        return show(id, [ring], () => {
          const r = target.getBoundingClientRect();
          Object.assign(ring.style, {
            left: `${r.left - pad}px`, top: `${r.top - pad}px`,
            width: `${r.width + 2 * pad}px`, height: `${r.height + 2 * pad}px`,
          });
        });
      },
      caption(text: string): number {
        const box = document.createElement("div");
        box.className = "caption fade";
        box.textContent = text;
        layer.appendChild(box);
        return show(nextId++, [box]);
      },
      card(title: string, subtitle: string): number {
        const card = document.createElement("div");
        card.className = "card fade";
        const h1 = document.createElement("h1");
        h1.textContent = title;
        card.appendChild(h1);
        if (subtitle) {
          const p = document.createElement("p");
          p.textContent = subtitle;
          card.appendChild(p);
        }
        layer.appendChild(card);
        return show(nextId++, [card]);
      },
      /**
       * An invisible element that sits on the first visible terminal row
       * containing `needle`, re-found every frame, so annotations can point
       * at a line of agent output. xterm draws text that has no element of
       * its own; the row's position comes from the terminal's own buffer and
       * cell size (the `__farhelmTerm` test surface the e2e helpers use).
       * Column positions assume one cell per character before the match,
       * which holds for the plain-text lines worth pointing at. Anchors live
       * until the page does. Returns the anchor's id.
       */
      terminalAnchor(needle: string): number {
        const id = nextId++;
        const anchor = document.createElement("div");
        anchor.className = "anchor";
        anchor.dataset.anchor = String(id);
        layer.appendChild(anchor);
        trackers.set(id, () => {
          const term = w.__farhelmTerm;
          const screen = term?.element?.querySelector(".xterm-screen") as HTMLElement | null;
          if (!term || !screen) return;
          const buffer = term.buffer.active;
          const rect = screen.getBoundingClientRect();
          const cw = rect.width / term.cols, ch = rect.height / term.rows;
          for (let row = 0; row < term.rows; row++) {
            const col = buffer.getLine(buffer.viewportY + row)?.translateToString(true).indexOf(needle) ?? -1;
            if (col < 0) continue;
            Object.assign(anchor.style, {
              left: `${rect.left + col * cw}px`, top: `${rect.top + row * ch}px`,
              width: `${needle.length * cw}px`, height: `${ch}px`,
            });
            return;
          }
        });
        trackers.get(id)?.();
        return id;
      },
      dismiss(id: number): void {
        trackers.delete(id);
        const elements = nodes.get(id) ?? [];
        nodes.delete(id);
        elements.forEach((el) => el.classList.remove("shown"));
        setTimeout(() => elements.forEach((el) => el.remove()), fadeMs + 50);
      },
      cursorTo(x: number, y: number, ms: number): void {
        cursor.style.transitionDuration = `${ms}ms`;
        cursor.style.opacity = "1";
        // The SVG's tip is at (3, 2); offset so the tip, not the box corner, lands on the point.
        cursor.style.transform = `translate(${x - 3}px, ${y - 2}px)`;
      },
      cursorPlace(x: number, y: number): void {
        cursor.style.transitionDuration = "0ms";
        cursor.style.transform = `translate(${x - 3}px, ${y - 2}px)`;
      },
      pulse(x: number, y: number): void {
        const ring = document.createElement("div");
        ring.className = "pulse";
        ring.style.left = `${x}px`;
        ring.style.top = `${y}px`;
        layer.appendChild(ring);
        setTimeout(() => ring.remove(), 600);
      },
    };
  };

  if (document.documentElement) start();
  else document.addEventListener("DOMContentLoaded", start, { once: true });
}

/**
 * The choreography's handle on the page: pointer, typing, and annotations,
 * each paced for a viewer rather than for a test. Every method resolves
 * once its motion or fade has finished on screen, so a choreography reads
 * as a sequence of beats without timing arithmetic of its own.
 */
export class Director {
  private cursor = { x: 0, y: 0 };

  private constructor(private readonly page: Page) {}

  /**
   * Install the overlay into the page now and into every later document,
   * and park the pointer near the bottom-right corner, hidden until the
   * first move.
   */
  static async install(page: Page): Promise<Director> {
    await page.addInitScript(installOverlay, FADE_MS);
    await page.evaluate(installOverlay, FADE_MS);
    const director = new Director(page);
    const viewport = page.viewportSize() ?? { width: 1280, height: 720 };
    director.cursor = { x: viewport.width * 0.82, y: viewport.height * 0.78 };
    await page.evaluate(({ x, y }) => (window as any).__farhelmDemo.cursorPlace(x, y), director.cursor);
    return director;
  }

  /** Hold the current picture for `ms`: a beat for the viewer to read or watch. */
  async hold(ms: number): Promise<void> {
    await this.page.waitForTimeout(ms); // sleep-ok: on-screen dwell for the video's viewer, not a readiness wait
  }

  /** The point a pointer action aims at: the target's centre, or `offset` from its top-left. */
  private async pointOf(target: Locator, offset?: { x: number; y: number }) {
    await target.scrollIntoViewIfNeeded();
    const box = await target.boundingBox();
    if (!box) throw new Error(`cannot point at ${target}: it has no box on screen`);
    return offset ? { x: box.x + offset.x, y: box.y + offset.y } : { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  }

  /**
   * Glide the pointer to `target`. Duration scales with distance so short
   * hops do not crawl and long ones do not teleport.
   */
  async moveTo(target: Locator, offset?: { x: number; y: number }): Promise<{ x: number; y: number }> {
    const point = await this.pointOf(target, offset);
    const distance = Math.hypot(point.x - this.cursor.x, point.y - this.cursor.y);
    const ms = Math.round(Math.min(1100, Math.max(350, distance * 0.9)));
    await this.page.evaluate(({ x, y, ms }) => (window as any).__farhelmDemo.cursorTo(x, y, ms), { ...point, ms });
    await this.hold(ms + 120);
    // The real mouse jumps only once the drawn pointer has arrived, so the
    // hover state the app draws appears with the pointer rather than ahead
    // of it. Rows the drawn pointer passes over on the way show no hover;
    // that reads as a smooth glide, where hover flicker along the path would
    // not.
    await this.page.mouse.move(point.x, point.y);
    this.cursor = point;
    return point;
  }

  /** Glide to `target`, pulse, and click exactly where the pointer is. */
  async click(target: Locator, offset?: { x: number; y: number }): Promise<void> {
    const point = await this.moveTo(target, offset);
    await this.page.evaluate(({ x, y }) => (window as any).__farhelmDemo.pulse(x, y), point);
    await this.page.mouse.click(point.x, point.y);
    await this.hold(250);
  }

  /** Type into whatever has focus, one key at a time at a human pace. */
  async type(text: string, delayMs = 75): Promise<void> {
    await this.page.keyboard.type(text, { delay: delayMs });
  }

  /** Press one key (Playwright key name) into whatever has focus. */
  async press(key: string): Promise<void> {
    await this.page.keyboard.press(key);
  }

  /** Draw a text box beside `target` with an arrow pointing at it. */
  async callout(target: Locator, text: string, placement: Placement = {}): Promise<Annotation> {
    const id = await target.evaluate(
      (el, { text, side, dx, dy }) => (window as any).__farhelmDemo.callout(el, text, side, dx, dy) as number,
      { text, side: placement.side ?? "auto", dx: placement.dx ?? 0, dy: placement.dy ?? 0 },
    );
    return this.annotation(id);
  }

  /**
   * A locator for the first visible line of the open terminal that contains
   * `needle`, usable as any annotation's or pointer action's target. The
   * terminal draws its text without an element per line, so this places an
   * invisible tracking element over it (see `terminalAnchor` in the page).
   * Fails if the text is not on screen when called.
   */
  async terminalText(needle: string): Promise<Locator> {
    const id = await this.page.evaluate((needle) => (window as any).__farhelmDemo.terminalAnchor(needle) as number, needle);
    const anchor = this.page.locator(`farhelm-demo-overlay [data-anchor="${id}"]`);
    const box = await anchor.boundingBox();
    if (!box || box.width === 0) throw new Error(`terminal text not on screen: ${needle}`);
    return anchor;
  }

  /** Ring `target`; `spotlight` also dims everything else. */
  async highlight(target: Locator, options: { spotlight?: boolean; pad?: number } = {}): Promise<Annotation> {
    const id = await target.evaluate(
      (el, { spotlight, pad }) => (window as any).__farhelmDemo.highlight(el, spotlight, pad) as number,
      { spotlight: options.spotlight ?? false, pad: options.pad ?? 6 },
    );
    return this.annotation(id);
  }

  /** A line of narration along the bottom of the frame. */
  async caption(text: string): Promise<Annotation> {
    const id = await this.page.evaluate((text) => (window as any).__farhelmDemo.caption(text) as number, text);
    return this.annotation(id);
  }

  /** A full-frame title card over a blurred view of the app. */
  async card(title: string, subtitle = ""): Promise<Annotation> {
    const id = await this.page.evaluate(
      ({ title, subtitle }) => (window as any).__farhelmDemo.card(title, subtitle) as number,
      { title, subtitle },
    );
    return this.annotation(id);
  }

  /** Wait for the fade-in, and hand back a dismiss that waits for the fade-out. */
  private async annotation(id: number): Promise<Annotation> {
    await this.hold(FADE_MS + 50);
    return {
      dismiss: async () => {
        await this.page.evaluate((id) => (window as any).__farhelmDemo.dismiss(id), id);
        await this.hold(FADE_MS + 50);
      },
    };
  }
}
