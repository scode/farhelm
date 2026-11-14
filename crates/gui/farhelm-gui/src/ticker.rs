//! The ticker: the one source of time for the GUI, for animation and for deadlines.
//!
//! SPEC_impl.md, "Animation time: the ticker". It serves two kinds of time:
//!
//! - Periodic animation ticks, at most [`TICKS_PER_SECOND`] a second, only while some view holds an [`Animating`]
//!   handle, only for views whose window is active, and never under reduced motion. A view that animates holds the
//!   handle and is notified on each tick; it never runs a timer of its own. Without one shared clock, a single
//!   animating row redraws its whole list at the display's rate (Zed's own agent sidebar pins a CPU core that way),
//!   and the old app had the same "nothing redraws on its own more than 10 times a second" rule.
//! - One-shot deadlines ([`after`]): a hover delay, a banner's expiry, a "still waiting" threshold. They fire on time
//!   whether or not the window is active, because they are about the user's time, not about drawing.
//!
//! GUI crates may not read the system clock or start timers (`crates/gui/clippy.toml`), so tests drive all of this
//! through GPUI's executor clock, and this module holds the crate's one exemption from the timer ban.
//!
//! Whether a window counts as active comes from GPUI, unless [`set_active_override`] says otherwise: headless Mutter
//! reports every window inactive forever (SPEC_impl.md, "GPUI cookbook: traps"), so the test-automation bridge's runs
//! pin it. In-process tests do not need the override: GPUI's test platform reports activity, and a test deactivates a
//! window with `VisualTestContext::deactivate_window`. Reduced motion is the operating system's setting, read by the
//! host process once at startup and handed in ([`set_reduced_motion`]); this crate does not query the platform itself.
//! Under reduced motion no tick loop runs at all, so nothing moves and nothing wakes ten times a second.
//!
//! NOTE: There is no animation frame counter yet. The first view that animates adds the phase it needs (a cursor blink,
//! a pulse) here, beside the tick that drives it.

use std::rc::{Rc, Weak};
use std::time::Duration;

use gpui::{AnyWindowHandle, App, Context, EntityId, Global, Task, Window};

/// The most animation ticks per second any view gets.
pub const TICKS_PER_SECOND: u32 = 10;

/// The time between two animation ticks.
pub const TICK: Duration = Duration::from_millis(1000 / TICKS_PER_SECOND as u64);

/// The ticker's state: who animates, and the one task that ticks while anyone does.
#[derive(Default)]
struct Ticker {
    animating: Vec<Subscriber>,
    /// The tick loop. It stops by itself once nobody animates; [`animate`] starts a new one, replacing (and so
    /// dropping) the finished task.
    tick_loop: Option<Task<()>>,
    running: bool,
    reduced_motion: bool,
    active_override: Option<bool>,
}

impl Global for Ticker {}

/// One view that animates: it is notified on each tick while its window is active and its handle lives.
struct Subscriber {
    alive: Weak<()>,
    view: EntityId,
    window: AnyWindowHandle,
}

/// Held by a view for as long as it animates. Dropping it stops that view's ticks.
#[must_use = "a view animates only while it holds this"]
pub struct Animating {
    _alive: Rc<()>,
}

/// A one-shot deadline. Dropping it cancels the callback, which is how a hover that ends cancels its tooltip.
#[must_use = "dropping a deadline cancels it"]
pub struct Deadline {
    _task: Task<()>,
}

/// Install the ticker. Part of the bootstrap.
pub fn install(cx: &mut App) {
    cx.set_global(Ticker::default());
}

/// The installed ticker, or `None` before [`install`] (a bare test fixture): checked first because `global_mut` panics
/// on a missing global, as the notice center and the layer manager also check. Without a ticker nothing ticks and the
/// setters change nothing.
fn installed(cx: &mut App) -> Option<&mut Ticker> {
    if cx.has_global::<Ticker>() {
        Some(cx.global_mut::<Ticker>())
    } else {
        None
    }
}

/// Record the operating system's reduced-motion setting. Under reduced motion views still draw their state, but no
/// animation ticks are delivered, so nothing moves on its own.
pub fn set_reduced_motion(reduced: bool, cx: &mut App) {
    if let Some(ticker) = installed(cx) {
        ticker.reduced_motion = reduced;
    }
}

/// Pin whether windows count as active (`Some`), or follow GPUI again (`None`). For runs where the platform cannot
/// report activity truthfully: headless Mutter, which reports every window inactive.
pub fn set_active_override(active: Option<bool>, cx: &mut App) {
    if let Some(ticker) = installed(cx) {
        ticker.active_override = active;
    }
}

/// Start animating the view `cx` belongs to, in `window`: it is notified on every tick until the handle drops. Under
/// reduced motion the handle is still returned, but no tick loop starts.
pub fn animate<V: 'static>(window: &Window, cx: &mut Context<V>) -> Animating {
    let alive = Rc::new(());
    let view = cx.entity_id();
    let Some(ticker) = installed(cx) else {
        return Animating { _alive: alive };
    };
    ticker.animating.push(Subscriber {
        alive: Rc::downgrade(&alive),
        view,
        window: window.window_handle(),
    });
    if !ticker.running && !ticker.reduced_motion {
        ticker.running = true;
        let task = cx.spawn(async move |_, cx| {
            loop {
                timer(cx.background_executor(), TICK).await;
                if !cx.update(tick) {
                    break;
                }
            }
        });
        if let Some(ticker) = installed(cx) {
            ticker.tick_loop = Some(task);
        }
    }
    Animating { _alive: alive }
}

/// Run `callback` once, `delay` from now, whether or not any window is active. Dropping the returned [`Deadline`]
/// cancels it.
pub fn after(delay: Duration, cx: &mut App, callback: impl FnOnce(&mut App) + 'static) -> Deadline {
    let executor = cx.background_executor().clone();
    let task = cx.spawn(async move |cx| {
        timer(&executor, delay).await;
        cx.update(callback);
    });
    Deadline { _task: task }
}

/// One tick: notify every live subscriber whose window is active. Returns whether the loop should keep running: not
/// once nobody animates, and not under reduced motion.
fn tick(cx: &mut App) -> bool {
    let (subscribers, override_active) = {
        let Some(ticker) = installed(cx) else {
            return false;
        };
        ticker
            .animating
            .retain(|subscriber| subscriber.alive.strong_count() > 0);
        if ticker.animating.is_empty() || ticker.reduced_motion {
            ticker.running = false;
            return false;
        }
        let subscribers: Vec<(EntityId, AnyWindowHandle)> = ticker
            .animating
            .iter()
            .map(|subscriber| (subscriber.view, subscriber.window))
            .collect();
        (subscribers, ticker.active_override)
    };
    for (view, window) in subscribers {
        let active = match override_active {
            Some(active) => active,
            None => window
                .update(cx, |_, window, _| window.is_window_active())
                .unwrap_or(false),
        };
        if active {
            cx.notify(view);
        }
    }
    true
}

/// The crate's one timer: everything time-based goes through [`animate`] or [`after`], which call this.
#[allow(
    clippy::disallowed_methods,
    reason = "the ticker is the one owner of GUI time (SPEC_impl.md, \"Animation time: the ticker\")"
)]
fn timer(executor: &gpui::BackgroundExecutor, duration: Duration) -> gpui::Task<()> {
    executor.timer(duration)
}
