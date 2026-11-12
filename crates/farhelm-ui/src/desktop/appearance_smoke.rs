//! Fixed native appearance evidence for the opt-in desktop smoke.
//!
//! The head probe only observes. The later eval uses ordinary Settings controls
//! and fixed real-terminal input, then returns one
//! bounded JSON record over the existing eval bridge. The four cases check
//! startup palettes, ordinary Settings choices and private-profile persistence.
//! A separate fixed rootless index exercises refusal before App initialization.
//! There is no externally supplied script or command channel. These are DOM
//! and integration assertions; normal appshots supply visual review evidence.

use anyhow::{Context, bail};
use dioxus::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

/// Select the four appearance boundaries or the fixed missing-initialize fault.
/// Invalid input refuses startup rather than silently using operator storage.
fn scenario() -> anyhow::Result<Option<String>> {
    match std::env::var("FARHELM_SMOKE_APPEARANCE") {
        Err(std::env::VarError::NotPresent) => Ok(None),
        Ok(value)
            if matches!(
                value.as_str(),
                "dark-fresh" | "dark-stored" | "light-fresh" | "light-stored" | "no-initialize"
            ) =>
        {
            Ok(Some(value))
        }
        _ => bail!("invalid FARHELM_SMOKE_APPEARANCE selection"),
    }
}

/// Whether to insert the passive early observer; configuration validates inputs.
pub(super) fn enabled() -> bool {
    matches!(scenario(), Ok(Some(_)))
}

/// Bind an explicit, existing private WebKit profile to this debug fixture.
///
/// Farhelm's own state directory does not isolate WebKit localStorage. The
/// smoke creates this directory and intentionally reuses it only for the
/// corresponding persistence relaunch. Missing or unusable inputs never fall
/// back to WebKit's default profile. This seam makes no macOS profile claim.
pub(super) fn configure(
    config: dioxus::desktop::Config,
) -> anyhow::Result<dioxus::desktop::Config> {
    let Some(selection) = scenario()? else {
        if std::env::var_os("FARHELM_SMOKE_WEBVIEW_DATA_DIR").is_some() {
            bail!("appearance smoke profile requires an appearance selection");
        }
        return Ok(config);
    };
    if selection == "no-initialize" && !cfg!(target_os = "linux") {
        bail!("missing-initialize smoke requires Linux");
    }
    let profile = PathBuf::from(
        std::env::var_os("FARHELM_SMOKE_WEBVIEW_DATA_DIR")
            .context("appearance smoke requires FARHELM_SMOKE_WEBVIEW_DATA_DIR")?,
    );
    if !profile.is_absolute() || !profile.is_dir() {
        bail!("appearance smoke profile must be an existing absolute directory");
    }
    let profile = profile
        .canonicalize()
        .context("resolving appearance smoke profile")?;
    let config = config.with_data_directory(profile);
    if selection == "no-initialize" {
        // Pinned Dioxus injects its ordinary loader before the explicit body
        // end tag, then sends initialize only if #main exists. A valid rootless
        // document exercises the independent creation deadline without a loader
        // monkeypatch or the panic caused by omitting that closing tag.
        eprintln!("desktop_no_initialize: configured rootless index");
        return Ok(config.with_custom_index(
            "<!doctype html><html><head><title>farhelm</title></head><body></body></html>".into(),
        ));
    }
    Ok(config)
}

/// Record real head state and the first text-bearing renderer mutation.
///
/// Kept after the exact appearance script and stylesheet and before the body
/// loader. Observations never write storage, appearance, media APIs or visible
/// content. The first-content boundary is DOM insertion, not a paint claim.
pub(super) const HEAD_OBSERVER: &str = r#"<script id="farhelm-appearance-smoke">
(() => {
  const snapshot = () => {
    let stored = null, storageError = null;
    try { stored = localStorage.getItem('farhelm.appearance'); }
    catch (error) { storageError = String(error); }
    const root = getComputedStyle(document.documentElement);
    return {
      at: performance.now(), root: document.documentElement.dataset.appearance,
      stored, storageError, nativeLight: matchMedia('(prefers-color-scheme: light)').matches,
      rootBackground: root.backgroundColor, ground: root.getPropertyValue('--bg-0').trim(),
      terminalBackground: root.getPropertyValue('--terminal-bg').trim(),
      bodyBackground: document.body ? getComputedStyle(document.body).backgroundColor : null,
      content: document.querySelector('#main')?.textContent.trim().slice(0, 160) || null,
    };
  };
  const state = window.__farhelmAppearanceSmoke = {
    timeOrigin: performance.timeOrigin, head: snapshot(), firstContent: null,
    headOrder: [...document.head.children].map(node => ({
      tag: node.tagName, id: node.id, src: node.getAttribute('src'),
    })),
  };
  const observer = new MutationObserver(() => {
    if (!document.querySelector('#main')?.textContent.trim()) return;
    state.firstContent = snapshot();
    observer.disconnect();
    requestAnimationFrame(() => { state.afterContentFrame = snapshot(); });
  });
  observer.observe(document.documentElement, {subtree: true, childList: true, characterData: true});
})();
</script>"#;

/// Observe the actual page once, without changing App's lifecycle or readiness.
/// The Rust timeout also catches a dead eval bridge; missing output is failure.
pub(super) fn use_observation() {
    use_hook(|| {
        let Some(selection) = scenario().expect("appearance smoke input validated before launch")
        else {
            return;
        };
        if selection == "no-initialize" {
            // App invokes this hook only after the renderer's initialize IPC.
            // The fault harness rejects this marker instead of treating the
            // ordinary driver's eventual timeout as proof that App never ran.
            eprintln!("desktop_no_initialize: unexpected App root");
            return;
        }
        spawn(async move {
            // The only substituted value comes from the closed scenario set.
            let script = format!("const scenario = {selection:?};\n{DRIVER}");
            let result = tokio::time::timeout(
                Duration::from_secs(75),
                document::eval(&script).join::<serde_json::Value>(),
            )
            .await;
            match result {
                Ok(Ok(value)) => eprintln!("desktop_appearance_smoke: {value}"),
                Ok(Err(error)) => eprintln!(
                    "desktop_appearance_smoke: {}",
                    serde_json::json!({"error": format!("eval: {error}")})
                ),
                Err(_) => {
                    eprintln!("desktop_appearance_smoke: {{\"error\":\"native eval timeout\"}}")
                }
            }
            // The diagnostic record can exceed one stderr write. Its prefix
            // is not a completion oracle: the collector must wait until the
            // entire JSON line has been written before taking its snapshot.
            eprintln!("desktop_appearance_smoke_done");
        });
    });
}

/// The finite product-control path shared by the four ordinary appearance cases.
///
/// Startup is observed before any interaction. The shell creates one ordinary
/// owned session through the authenticated API; opening its real row gives
/// terminal evidence without adding a permanent application fixture feature.
/// The app creates every session in a terminal host, so the terminal is the
/// grid component, and the probe reads it as the browser suite's terminal
/// helpers do: its painted background, its input field for focus, its own
/// `paste` and grid `text` input messages for typing, and its model's
/// `snapshot()` (history and screen) for the output. A terminal that is not
/// a grid fails the case rather than being skipped. Input and a numbered
/// output burst must retain that same live attachment. Polling names each
/// readiness boundary and has one overall deadline.
const DRIVER: &str = r#"
return await (async () => {
  const evidence = {scenario, phases: [], coverageGaps: ['live native system appearance event not supplied by this fixture']};
  const deadline = performance.now() + 65000;
  const check = (condition, message) => { if (!condition) throw new Error(message); };
  const wait = async (read, name) => {
    while (performance.now() < deadline) {
      const result = read();
      if (result) return result;
      // sleep-ok: bounded polling of the named DOM/native-property oracle.
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    throw new Error('timed out waiting for ' + name);
  };
  try {
    const early = window.__farhelmAppearanceSmoke;
    check(early, 'head observer missing');
    evidence.early = early;
    const lightSystem = scenario.startsWith('light-');
    const stored = scenario.endsWith('-stored');
    const override = lightSystem ? 'dark' : 'light';
    const initial = stored ? override : (lightSystem ? 'light' : 'dark');
    check(early.head.nativeLight === lightSystem, 'actual native matchMedia does not establish requested system premise');
    check(!early.head.storageError, 'private profile localStorage read failed');
    check(early.head.stored === (stored ? override : null), 'unexpected initial private-profile choice');
    check(early.head.root === initial, 'wrong resolved appearance in head');
    const canvases = {dark: 'rgb(24, 28, 36)', light: 'rgb(244, 246, 250)'};
    check(early.head.rootBackground === canvases[initial], 'head canvas does not use the resolved Steel palette');
    const order = early.headOrder.map(node => node.id);
    check(order.indexOf('farhelm-appearance') >= 0 && order.indexOf('farhelm-app-css') > order.indexOf('farhelm-appearance'), 'head appearance/CSS order');
    check(!early.head.content && early.head.bodyBackground === null, 'head probe ran after body content');
    await wait(() => early.firstContent, 'first text-bearing native renderer content');
    check(early.firstContent.root === initial, 'first native renderer content has wrong appearance');
    check(early.firstContent.rootBackground === canvases[initial] && early.firstContent.bodyBackground === canvases[initial], 'first native renderer canvas has wrong palette');
    await wait(() => document.querySelector('.app-settings-toggle'), 'authenticated Settings toggle');
    // A fresh owned state opens the same Welcome card as an ordinary install.
    // Dismiss it through its real control before using the underlying app.
    const welcome = document.querySelector('.welcome-got-it');
    if (welcome) {
      welcome.click();
      await wait(() => !document.querySelector('.welcome-dialog'), 'Welcome dismissal');
    }
    const row = await wait(() => document.querySelector('.session-row-open'), 'owned terminal fixture row');
    const sessionId = row.closest('[data-session-id]')?.dataset.sessionId;
    check(sessionId, 'owned terminal row has no session identity');
    row.click();
    // The page draws every terminal with the grid component.
    await wait(() => window.__farhelmGrid, 'mounted grid terminal');
    // The grid paints its theme's background at its first frame, which the
    // reveal flushes; every snapshot below measures that paint.
    await wait(() => window.__farhelmIslands?.terminal?.test?.replay?.revealed, 'revealed grid terminal');
    check(getComputedStyle(window.__farhelmGrid.element).backgroundColor !== 'rgba(0, 0, 0, 0)', 'grid terminal never painted');
    const open = async () => {
      document.querySelector('.app-settings-toggle').click();
      await wait(() => document.querySelector('.app-settings-dialog'), 'Settings dialog');
      const nav = await wait(() => document.querySelector('button[aria-controls="settings-appearance"]'), 'Appearance navigation');
      nav.click();
      await wait(() => document.querySelector('#settings-appearance:not([hidden])'), 'visible Appearance section');
      await wait(() => document.querySelector('[data-appearance-choice="' + (stored ? override : 'system') + '"]').checked, 'initial native radio property');
    };
    // The terminal keeps its own dark background whatever the app's
    // appearance: the token the stylesheet defines, and the color the grid
    // paints on its element from the terminal theme.
    const terminalDark = {token: '#282c34', painted: 'rgb(40, 44, 52)'};
    const snapshot = label => {
      const root = getComputedStyle(document.documentElement);
      const grid = window.__farhelmGrid;
      const phase = {label, at: performance.now(), timeOrigin: performance.timeOrigin,
        nativeLight: matchMedia('(prefers-color-scheme: light)').matches,
        root: document.documentElement.dataset.appearance, stored: localStorage.getItem('farhelm.appearance'),
        ground: root.getPropertyValue('--bg-0').trim(), canvas: root.backgroundColor,
        checked: [...document.querySelectorAll('[data-appearance-choice]')].filter(node => node.checked).map(node => node.dataset.appearanceChoice),
        terminal: grid ? {background: getComputedStyle(grid.element).backgroundColor, token: root.getPropertyValue('--terminal-bg').trim(), screens: document.querySelectorAll('.grid-terminal-screen').length} : null};
      evidence.phases.push(phase);
      check(phase.timeOrigin === early.timeOrigin, 'appearance action reloaded the native document');
      check(phase.canvas === canvases[phase.root], 'canvas does not follow the resolved palette');
      check(phase.terminal, 'grid terminal gone');
      check(phase.terminal.background === terminalDark.painted && phase.terminal.token.toLowerCase() === terminalDark.token, 'terminal dark background changed');
      return phase;
    };
    await open();
    snapshot('initial');
    const choose = async choice => {
      const radio = document.querySelector('[data-appearance-choice="' + choice + '"]');
      check(radio?.type === 'radio', 'appearance choice is not a native radio');
      radio.click();
      const expected = choice === 'system' ? (lightSystem ? 'light' : 'dark') : choice;
      await wait(() => radio.checked && document.documentElement.dataset.appearance === expected && localStorage.getItem('farhelm.appearance') === choice, 'applied and persisted ' + choice);
      // Give each ordinary choice a rendering turn before inspecting settled state.
      // This is not a compositor or atomic-presentation assertion.
      await new Promise(resolve => requestAnimationFrame(resolve));
      const phase = snapshot(choice);
      check(phase.checked.length === 1 && phase.checked[0] === choice, 'native radio group is out of sync');
      check(phase.nativeLight === lightSystem, 'radio action changed native system signal');
    };
    for (const choice of ['light', 'dark', 'system', override]) await choose(choice);
    document.querySelector('.app-settings-close').click();
    await wait(() => !document.querySelector('.app-settings-dialog'), 'Settings close');
    document.querySelector('.app-settings-toggle').click();
    await wait(() => document.querySelector('[data-appearance-choice="' + override + '"]')?.checked, 'remounted native radio property');
    snapshot('remounted');
    document.querySelector('.app-settings-close').click();
    await wait(() => !document.querySelector('.app-settings-dialog'), 'Settings close before terminal');
    await document.fonts.load('14px Inter');
    await document.fonts.ready;
    evidence.fonts = {ready: document.fonts.check('14px Inter'), interfaceFamily: getComputedStyle(document.querySelector('.app-settings-toggle')).fontFamily,
      interFaces: [...document.fonts].filter(face => face.family.replaceAll('"', '') === 'Inter').map(face => ({family: face.family, weight: face.weight, status: face.status}))};
    check(evidence.fonts.ready && evidence.fonts.interfaceFamily.includes('Inter') && evidence.fonts.interFaces.some(face => face.status === 'loaded'), 'vendored Inter not loaded for interface text');

    // Mount alone is not live input readiness. Pin the requested session's
    // actual socket and replay reveal, then keep this same attachment through
    // the round trip; reconnecting must not silently rescue a failed burst.
    const island = await wait(() => {
      const current = window.__farhelmIslands?.terminal;
      if (!current?.grid || current.ws?.readyState !== WebSocket.OPEN || !current.test?.replay?.revealed) return;
      const url = new URL(current.ws.url, location.href);
      const parts = url.pathname.split('/');
      const matches = decodeURIComponent(parts[3] || '') === sessionId && parts[1] === 'api' && parts[2] === 'sessions'
        && parts[4] === 'term' && (parts.length === 5 || (parts.length === 6 && parts[5] === 'unowned'))
        && !url.searchParams.has('tab');
      return matches ? current : null;
    }, 'live revealed terminal for the owned session');
    const grid = island.grid, socket = island.ws;
    const sameAttachment = () => {
      check(window.__farhelmIslands?.terminal === island && island.grid === grid && island.ws === socket
        && socket.readyState === WebSocket.OPEN && island.test.replay.revealed, 'terminal attachment closed or changed during input/burst');
    };
    grid.focus();
    const field = grid.element.querySelector('.grid-terminal-input');
    await wait(() => { sameAttachment(); return document.activeElement === field; }, 'grid terminal input focus');
    // Read the last 640 lines of the grid's model (history, then the screen),
    // not viewport-only DOM, and only once its history has been applied: the
    // grid applies a large history in slices, so a tail read mid-way would
    // miss lines that did arrive. The 512-line burst fits well within the
    // 12,000-line scrollback.
    const terminalLines = () => {
      sameAttachment();
      const shown = grid.snapshot();
      if (shown.historyPending) return null;
      return [...shown.history, ...shown.lines].slice(-640);
    };
    // Enter as the browser suite's `sendTerminalInput` sends it: a grid `text`
    // input message straight down the attachment's socket, which the
    // supervisor writes as the same byte (the grid itself would send a
    // pressed Enter as a `key` message).
    const pressEnter = () => {
      sameAttachment();
      const messages = window.farhelmGridInput.textMessages('\r').map(message => window.farhelmGridInput.encodeInput(message));
      check(messages.length > 0 && messages.every(Boolean), 'grid text message for Enter not encodable');
      for (const bytes of messages) socket.send(bytes);
    };
    const before = await wait(terminalLines, 'grid history applied before input');
    check(!before.some(line => line === 'FH-NATIVE-INPUT' || line === 'FH-NATIVE-DONE' || line === 'FH-NATIVE-AFTER'), 'terminal markers already present before input');
    const inputAt = performance.now();
    // The grid's own paste sends `paste` input messages down the socket to
    // the PTY. Split printf markers cannot be satisfied by the shell echoing
    // the pasted line. This establishes terminal input, not physical native
    // keyboard dispatch.
    const command = "printf '\\nFH-NATIVE-%s\\n' INPUT; for ((i=1;i<=512;i++)); do printf 'FH-BURST-%04d\\n' \"$i\"; done; printf 'FH-NATIVE-%s\\n' DONE";
    check(grid.paste(command), 'grid terminal refused the pasted command');
    pressEnter();
    const burstLines = await wait(() => {
      const lines = terminalLines();
      return lines && lines.includes('FH-NATIVE-DONE') ? lines : null;
    }, 'owned terminal input acknowledgement and burst completion');
    const burst = burstLines.filter(line => /^FH-BURST-\d{4}$/.test(line));
    check(burstLines.includes('FH-NATIVE-INPUT'), 'terminal input acknowledgement missing');
    check(burst.length === 512 && burst.every((line, index) => line === 'FH-BURST-' + String(index + 1).padStart(4, '0')), 'terminal burst lost, duplicated or reordered lines');
    check(grid.paste("printf '\\nFH-NATIVE-%s\\n' AFTER"), 'grid terminal refused the second pasted command');
    pressEnter();
    const afterLines = await wait(() => {
      const lines = terminalLines();
      return lines && lines.includes('FH-NATIVE-AFTER') ? lines : null;
    }, 'terminal input after burst');
    evidence.terminalRoundtrip = {input: 'grid paste and text input messages', sessionId, sameAttachment: true,
      revealed: island.test.replay.revealed, socketOpen: socket.readyState === WebSocket.OPEN,
      burstLines: burst.length, first: burst[0], last: burst[burst.length - 1],
      elapsedMs: performance.now() - inputAt, afterAcknowledged: true, tail: afterLines.slice(-8)};
    const final = snapshot('terminal-final');
    check(final.root === override, 'final opposite-system override was lost');
    evidence.ok = true;
  } catch (error) { evidence.error = String(error); evidence.ok = false; }
  return evidence;
})();
"#;
