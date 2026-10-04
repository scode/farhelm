// Farhelm's own hover tooltip: a short text that appears 300 ms after the
// pointer comes onto a control, or a control gains keyboard focus, and sits
// directly above it.
//
// ## Why not the browser's `title` tooltip
//
// The native tooltip's delay belongs to the browser engine. No attribute,
// style or script can shorten it, it is a second or more in every engine,
// and WebKit (which the macOS desktop app embeds) ignores the macOS
// tooltip-delay default as well. Hover help that takes that long is hover
// help nobody discovers. So nothing in the UI uses `title` as hover help;
// elements opt in to this tooltip with a `data-tooltip` attribute instead,
// and a `title` would bring the slow native box back on top of this one.
//
// ## Shape
//
// One delegated listener set on `document` and one tooltip element appended
// to `body`. Dioxus components set `data-tooltip` like any other attribute,
// and a script can set it on elements it creates, so the mechanism stays out
// of every component's props. The listeners run in the capture phase so
// a component that stops propagation of a pointer or key event cannot hide
// that event from the tooltip.
//
// The element is `position: fixed` and positioned from the target's
// measured rectangle, never anchored inside the target. That is what keeps
// it from being clipped by a scroll container: `.app-sidebar` is
// `overflow: hidden auto`, which clips anything positioned inside a row.
// The text is set with `textContent`, never as markup, so peer-supplied
// text (session titles, paths, agent invocations) stays text; the Rust side
// still passes it through `display_peer` first, exactly as it did for the
// native tooltips this replaces.
//
// ## When it shows and hides
//
// - Shows 300 ms after the pointer comes to rest on a target (each movement
//   inside it starts the wait over), or after a target gains focus that
//   matches `:focus-visible` (keyboard navigation, not a click).
// - Shows at once, with no new delay, when another target is entered while
//   one is showing or within 300 ms of the pointer (or keyboard focus)
//   leaving one, so a user
//   scanning a row of icons is not made to wait at each. Only moving off a
//   target opens that window: after a press, Escape, a scroll
//   or the window losing focus, the next tooltip waits the full delay again,
//   since the user has just put one away or the page moved under a resting
//   pointer.
// - Hides when the pointer leaves the target, on any pointer press, on
//   Escape, when a focus-shown target loses focus, when the window loses
//   focus or is hidden, when the target leaves the document, and on a scroll
//   of the document or of a container holding the target. Scrolls elsewhere
//   (the terminal scrolling under live output while a header button is
//   hovered) leave it alone.
// - A press or Escape keeps that element's tooltip down until the pointer
//   leaves it or it loses focus, as the native tooltip does, so a click does
//   not bring it straight back.
// - Touch input never shows it: a tap has no hover, and a tooltip left over
//   from a tap would cover what the user is touching.
//
// A removed target is detected by checking `isConnected` on a short timer
// while a tooltip is pending or showing, rather than with a DOM-change
// observer: Dioxus rewrites the DOM constantly, and an observer on the whole
// tree would run on every live status update for the sake of one tooltip.
// The same tick picks up a `data-tooltip` whose text changed under a
// showing tooltip (a status dot that moved from running to waiting), and a
// target that moved without any scroll (the sidebar re-sorting on a status
// change, the header reflowing, the window resizing), so the tooltip follows
// it instead of staying beside whatever took its place.
//
// The element is `aria-hidden`: it is a visual aid, and nothing references
// it, so a screen reader would only meet it as stray text at the end of the
// page. Anything a control's hover text says that assistive technology also
// needs is carried by the control itself (an `aria-label`, a
// `.visually-hidden` copy, an `aria-describedby` target).
//
// ## Placement
//
// Above the target, about 6 px away and horizontally centred, shifted
// sideways to stay inside the window; below only when there is no room
// above, and then with a 28 px gap. The asymmetry is deliberate. A page
// cannot know the cursor's size, and the standard arrow and pointing-hand
// cursors extend downward from their hot spot, so the space above a control
// is never under the cursor whatever its size, while a box just below a
// 16 px icon would sit under the cursor's body. `placeTooltip` is a pure
// function, exported for js-tests/tooltip.test.js, so this rule is tested
// without a browser.
//
// ## Testability
//
// The pure decisions (`placeTooltip`, `instantFollowOn`, `scrollMovesTarget`)
// are exported under CommonJS as well as installed on `window`, the same
// dual install term-bytes.js uses. Everything stateful is exercised by the
// browser suite (e2e/tests/tooltip.spec.ts), which is where the wiring to
// real pointer, focus and scroll events can actually be observed.
(function () {
  /** How long the pointer rests on a target, or keyboard focus stays on one, before the tooltip shows. */
  const SHOW_DELAY_MS = 300;
  /** How soon after one tooltip hides another target still gets its tooltip at once. */
  const FOLLOW_ON_GRACE_MS = 300;
  /** Gap between the target's top edge and the tooltip's bottom when placed above. */
  const GAP_ABOVE = 6;
  /** Gap below the target when there is no room above; clears the cursor's body. */
  const GAP_BELOW = 28;
  /** Closest the tooltip may come to a window edge. */
  const EDGE = 4;
  /** How often a pending or showing tooltip re-checks its target (removal, new text). */
  const WATCH_MS = 200;

  /**
   * Where the tooltip goes for a target rectangle, a tooltip size and a
   * viewport size, all in CSS pixels in viewport coordinates.
   *
   * Returns `{left, top, side}` with `side` either `"above"` or `"below"`.
   * Above wins whenever the tooltip fits between the window's top edge and
   * the target; below is the fallback. When it fits on neither side (a
   * tooltip taller than the room either way, which the stylesheet's
   * maximum height keeps rare) it takes the side with more room and is
   * clamped inside the window, overlapping the target rather than leaving
   * the screen. Horizontally it is centred on the target, then shifted to
   * stay at least `EDGE` from both window edges; a tooltip wider than the
   * window is pinned to the left edge, where its text starts.
   */
  function placeTooltip(target, size, viewport) {
    const centre = (target.left + target.right) / 2;
    let left = centre - size.width / 2;
    left = Math.min(left, viewport.width - EDGE - size.width);
    left = Math.max(EDGE, left);

    const aboveTop = target.top - GAP_ABOVE - size.height;
    if (aboveTop >= EDGE) return { left, top: aboveTop, side: "above" };
    const belowTop = target.bottom + GAP_BELOW;
    if (belowTop + size.height <= viewport.height - EDGE) {
      return { left, top: belowTop, side: "below" };
    }
    const roomAbove = target.top;
    const roomBelow = viewport.height - target.bottom;
    const side = roomAbove >= roomBelow ? "above" : "below";
    const preferred = side === "above" ? aboveTop : belowTop;
    const top = Math.max(EDGE, Math.min(preferred, viewport.height - EDGE - size.height));
    return { left, top, side };
  }

  /**
   * Whether a newly entered target should show its tooltip without the
   * usual delay: yes while one is showing, and for `FOLLOW_ON_GRACE_MS`
   * after the last one hid.
   */
  function instantFollowOn(now, lastHiddenAt, showing) {
    return showing || now - lastHiddenAt < FOLLOW_ON_GRACE_MS;
  }

  /**
   * Whether a scroll event whose target is `scrolled` can have moved
   * `target`: the document scrolling, or a container that holds it. A
   * scroll anywhere else (the terminal's own viewport under live output)
   * cannot move a header button, and hiding on it would make the tooltip
   * flicker away for no reason.
   */
  function scrollMovesTarget(scrolled, target, doc) {
    if (!scrolled || scrolled === doc || scrolled === doc.documentElement || scrolled === doc.body) {
      return true;
    }
    return typeof scrolled.contains === "function" && scrolled.contains(target);
  }

  /**
   * The tooltip target for an event target: the nearest element at or
   * above it carrying a non-empty `data-tooltip`, or null. Nearest wins, so
   * an icon with its own text inside a button with another shows its own.
   */
  function tooltipTarget(node) {
    let el = node && node.nodeType === 1 ? node : node && node.parentElement;
    while (el) {
      const text = el.getAttribute("data-tooltip");
      if (text !== null && text.trim() !== "") return el;
      el = el.parentElement;
    }
    return null;
  }

  /** `:focus-visible`, with a false answer from an engine that cannot evaluate it. */
  function focusVisible(el) {
    try {
      return el.matches(":focus-visible");
    } catch (_error) {
      return false;
    }
  }

  /**
   * Wire the tooltip into a page. Idempotent per window: `AppBody`
   * remounts whenever the authenticated tree is rebuilt, and a second
   * listener set would show two tooltips' worth of timers for one hover.
   */
  function install(win) {
    if (win.__farhelmTooltipInstalled) return;
    win.__farhelmTooltipInstalled = true;
    const doc = win.document;

    let tip = null;
    // The element the tooltip currently belongs to, shown or still pending.
    let target = null;
    let shown = false;
    // Whether `target` came from keyboard focus rather than the pointer.
    let viaFocus = false;
    let showTimer = null;
    let watchTimer = null;
    let lastHiddenAt = -Infinity;
    // The target's rectangle at the last placement, which `watch` compares
    // against to follow a target that moved without a scroll.
    let placedAt = { left: 0, top: 0, width: 0, height: 0 };
    // The element a press or Escape dismissed, kept down until the pointer leaves it.
    let suppressed = null;

    function element() {
      if (!tip) {
        tip = doc.createElement("div");
        tip.className = "farhelm-tooltip";
        tip.setAttribute("role", "tooltip");
        tip.setAttribute("aria-hidden", "true");
        tip.hidden = true;
      }
      // Re-attached on every show rather than once: nothing in the app
      // removes it today, but a page that replaced `body`'s children would
      // otherwise leave every later tooltip measuring a detached box.
      if (!tip.isConnected && doc.body) doc.body.appendChild(tip);
      return tip;
    }

    function stopTimers() {
      if (showTimer !== null) win.clearTimeout(showTimer);
      if (watchTimer !== null) win.clearInterval(watchTimer);
      showTimer = null;
      watchTimer = null;
    }

    // `warm` is true only when the pointer moved off the target, the one
    // kind of hide that opens the instant follow-on window (module docs).
    function hide(warm = false) {
      stopTimers();
      if (shown) {
        tip.hidden = true;
        if (warm) lastHiddenAt = win.performance.now();
      }
      shown = false;
      target = null;
      viaFocus = false;
    }

    function position(text) {
      const box = element();
      box.textContent = text;
      box.hidden = false;
      // Measure at the origin first, so the width it wraps to is its own
      // maximum width rather than whatever room was left at the previous
      // target's position. Moving it within the same task means nothing is
      // painted at the origin.
      box.style.left = "0px";
      box.style.top = "0px";
      const size = box.getBoundingClientRect();
      const rect = target.getBoundingClientRect();
      placedAt = { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
      const place = placeTooltip(
        rect,
        { width: size.width, height: size.height },
        { width: win.innerWidth, height: win.innerHeight },
      );
      box.style.left = `${place.left}px`;
      box.style.top = `${place.top}px`;
      box.dataset.side = place.side;
    }

    function currentText() {
      const text = target && target.getAttribute("data-tooltip");
      return text === null || text === undefined ? "" : text.trim();
    }

    function show() {
      showTimer = null;
      const text = currentText();
      if (!target || !target.isConnected || text === "") {
        hide();
        return;
      }
      position(text);
      shown = true;
    }

    function watch() {
      if (!target || !target.isConnected) {
        hide();
        return;
      }
      if (!shown) return;
      const text = currentText();
      if (text === "") {
        hide();
        return;
      }
      const rect = target.getBoundingClientRect();
      const moved = rect.left !== placedAt.left || rect.top !== placedAt.top
        || rect.width !== placedAt.width || rect.height !== placedAt.height;
      if (moved || text !== tip.textContent) position(text);
    }

    function schedule(next, focus) {
      if (next === target) {
        // Keyboard focus landing on the element the pointer already shows
        // makes the tooltip focus-owned, so the pointer leaving it does not
        // take it away while the element is still focused.
        viaFocus = viaFocus || focus;
        return;
      }
      const instant = instantFollowOn(win.performance.now(), lastHiddenAt, shown);
      stopTimers();
      target = next;
      viaFocus = focus;
      watchTimer = win.setInterval(watch, WATCH_MS);
      if (instant) show();
      else {
        if (shown) {
          tip.hidden = true;
          shown = false;
        }
        showTimer = win.setTimeout(show, SHOW_DELAY_MS);
      }
    }

    doc.addEventListener(
      "pointerover",
      (event) => {
        if (event.pointerType === "touch") {
          hide();
          return;
        }
        if (suppressed && !suppressed.contains(event.target)) suppressed = null;
        const next = tooltipTarget(event.target);
        if (next === null) {
          if (!viaFocus) hide(true);
          return;
        }
        if (next === suppressed) {
          // Back on a dismissed element from a target nested inside it:
          // the nested target's tooltip goes, and the dismissed one stays
          // down.
          if (!viaFocus && target !== null && target !== suppressed) hide(true);
          return;
        }
        schedule(next, false);
      },
      true,
    );

    // The delay counts from the pointer coming to rest, not from it
    // arriving: while a cold (not yet shown) tooltip is pending, every
    // movement inside its target starts the 300 ms over. A pointer sweeping
    // across a wide row on its way elsewhere therefore shows nothing, and the
    // tooltip appears once the user actually stops on something. Keyboard
    // focus and the warm follow-on never wait on this timer.
    doc.addEventListener(
      "pointermove",
      (event) => {
        if (showTimer === null || viaFocus || event.pointerType === "touch") return;
        if (target === null || !target.contains(event.target)) return;
        win.clearTimeout(showTimer);
        showTimer = win.setTimeout(show, SHOW_DELAY_MS);
      },
      true,
    );

    // Only leaving the window needs its own handler: moving between
    // elements inside the page always fires `pointerover` on the new one,
    // which the handler above already settles.
    doc.addEventListener(
      "pointerout",
      (event) => {
        if (event.relatedTarget !== null) return;
        suppressed = null;
        if (!viaFocus) hide(true);
      },
      true,
    );

    doc.addEventListener(
      "pointerdown",
      (event) => {
        suppressed = tooltipTarget(event.target);
        hide();
      },
      true,
    );

    doc.addEventListener(
      "keydown",
      (event) => {
        if (event.key !== "Escape" || target === null) return;
        suppressed = target;
        hide();
      },
      true,
    );

    doc.addEventListener(
      "focusin",
      (event) => {
        const next = tooltipTarget(event.target);
        // A text field matches `:focus-visible` even when clicked, so the
        // press that focused it is what keeps its tooltip down here.
        if (next === null || next === suppressed || !focusVisible(event.target)) return;
        schedule(next, true);
      },
      true,
    );

    doc.addEventListener(
      "focusout",
      (event) => {
        // A dismissal lasts while the dismissed element keeps focus. Without
        // this, a keyboard user who pressed Escape on a control, tabbed away
        // and came back would never see its tooltip again, because only a
        // pointer leaving the element cleared it before.
        if (suppressed !== null && suppressed.contains(event.target)) suppressed = null;
        // Focus moving on is the keyboard's version of the pointer moving
        // on, so it opens the same follow-on window: tabbing along a row of
        // controls shows each tooltip at once after the first.
        if (viaFocus && target !== null && target.contains(event.target)) hide(true);
      },
      true,
    );

    doc.addEventListener(
      "scroll",
      (event) => {
        if (target !== null && scrollMovesTarget(event.target, target, doc)) hide();
      },
      true,
    );

    win.addEventListener("blur", () => hide());
    doc.addEventListener("visibilitychange", () => {
      if (doc.visibilityState === "hidden") hide();
    });
  }

  const api = { placeTooltip, instantFollowOn, scrollMovesTarget, install };
  if (typeof window !== "undefined") {
    window.farhelmTooltip = api;
    install(window);
  }
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})();
