//! The ticker: the one source of time for the GUI, for animation and for deadlines.
//!
//! SPEC_impl.md, "Animation time: the ticker". It serves two kinds of time:
//!
//! - Periodic animation ticks, at most [`TICKS_PER_SECOND`] a second, only while some view holds an [`Animating`],
//!   [`Blinking`] or [`Pulsing`] handle, only for views whose window is active, and never under reduced motion. A view
//!   that animates holds a handle and is notified on the ticks it needs; it never runs a timer of its own. Without one
//!   shared clock, a single animating row redraws its whole list at the display's rate (Zed's own agent sidebar pins a
//!   CPU core that way), and the old app had the same "nothing redraws on its own more than 10 times a second" rule.
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
//! The ticks are counted, and an animation whose phase changes more slowly than every tick says how often it needs to
//! hear about it. A cursor blink ([`blink`]) changes twice a second, so its view is notified twice a second rather than
//! ten times, and asks its [`Blinking`] handle which half of the blink it is in; each blink starts on its own shown
//! half. The running pulse ([`pulse`]) is the other phase, and it is shared: every [`Pulsing`] handle reads the same
//! step of one cycle counted from the tick count, so every pulsing dot in the app steps together. A new phase goes here
//! too, beside the tick that drives it.

use std::rc::{Rc, Weak};
use std::time::Duration;

use gpui::{AnyWindowHandle, App, Context, EntityId, Global, Task, Window};

use crate::bootstrap::{installed, installed_mut};

/// The most animation ticks per second any view gets.
pub const TICKS_PER_SECOND: u32 = 10;

/// The time between two animation ticks.
pub const TICK: Duration = Duration::from_millis(1000 / TICKS_PER_SECOND as u64);

/// How many ticks each half of a cursor blink lasts: half a second shown, half a second hidden, the old terminal's
/// one-second `step-end` blink (`.grid-terminal-cursor-blink` in app.css at the reference commit).
pub const BLINK_HALF_TICKS: u64 = 5;

/// How many steps one cycle of the running pulse takes: the old `farhelm-status-pulse` keyframes (app.css at the
/// reference commit) under `steps(4)`, which applies per keyframe interval, so four steps from full brightness down to
/// the minimum and four back, five levels in all. Step 0 is full brightness and step `PULSE_STEPS / 2` the dimmest.
pub const PULSE_STEPS: u64 = 8;

/// How many ticks one pulse cycle lasts: the old animation's two seconds. Its 250 ms steps do not land on 100 ms
/// ticks, so a step lasts three ticks and the next two, in turn. That keeps the two-second cycle and exactly four
/// changes in any second, and puts no step boundary more than 50 ms from where the stylesheet had it. The other way to
/// keep whole ticks, 300 ms steps, would have stretched the cycle to 2.4 s.
pub const PULSE_CYCLE_TICKS: u64 = 20;

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
    /// How many ticks have been delivered since the app started. Only differences between two readings mean anything;
    /// the count stands still while nothing animates.
    ticks: u64,
}

impl Global for Ticker {}

/// One view that animates: it is notified on the ticks its [`Cadence`] picks, while its window is active and its
/// handle lives.
struct Subscriber {
    alive: Weak<()>,
    view: EntityId,
    window: AnyWindowHandle,
    cadence: Cadence,
    /// The tick count when the subscriber started, which [`Cadence::Every`] counts from.
    since: u64,
}

/// Which ticks a subscriber hears about.
#[derive(Clone, Copy)]
enum Cadence {
    /// Every this many ticks, counted from the subscriber's own start: each one's phase is its own.
    Every(u64),
    /// Each tick on which the shared pulse phase ([`pulse_step`]) moves to its next step, whenever the subscriber
    /// started: the phase is everyone's.
    PulseStep,
}

impl Cadence {
    /// Whether tick number `now` (at least 1: the count moves before anyone is notified) is one this cadence is
    /// notified on, for a subscriber that started at tick count `since`.
    fn due(self, now: u64, since: u64) -> bool {
        match self {
            Cadence::Every(every) => (now - since).is_multiple_of(every),
            Cadence::PulseStep => pulse_step(now) != pulse_step(now - 1),
        }
    }
}

/// The shared pulse's step at tick count `ticks`, `0..PULSE_STEPS`: step `k` covers the ticks from `k * 2.5`, rounded
/// up, within each cycle ([`PULSE_CYCLE_TICKS`]).
fn pulse_step(ticks: u64) -> u64 {
    (ticks * PULSE_STEPS / PULSE_CYCLE_TICKS) % PULSE_STEPS
}

/// Held by a view for as long as it animates. Dropping it stops that view's ticks.
#[must_use = "a view animates only while it holds this"]
pub struct Animating {
    _alive: Rc<()>,
}

/// Held by a view whose cursor blinks, for as long as it blinks. Dropping it stops the view's blink ticks; making a new
/// one starts the blink over on its shown half, which is how a terminal that takes focus shows its cursor at once.
#[must_use = "a view blinks only while it holds this"]
pub struct Blinking {
    _alive: Rc<()>,
    since: u64,
}

impl Blinking {
    /// Whether the blink is in its shown half now. Always shown under reduced motion, so a cursor never stays hidden
    /// because the blink stopped in its hidden half. The view is notified each time this changes, while its window is
    /// active; in an inactive window the answer may change unannounced, so a view there should not draw from it.
    pub fn is_on(&self, cx: &App) -> bool {
        match installed::<Ticker>(cx) {
            Some(ticker) if !ticker.reduced_motion => {
                (ticker.ticks.saturating_sub(self.since) / BLINK_HALF_TICKS).is_multiple_of(2)
            }
            _ => true,
        }
    }
}

/// Held by a view that draws the running pulse, for as long as it draws one. Dropping it stops the view's pulse ticks.
///
/// One handle serves any number of pulsing marks in the view, since the phase is shared: the view is notified once
/// per step, so it redraws at most four times a second however many marks it draws. A view must not hold one while
/// it draws no pulsing mark, because the notifications would redraw its window for nothing, nor while its window is
/// inactive ([`is_active`]): no notification reaches it there, but a live handle keeps the tick loop's timer firing
/// ten times a second for a window nobody is looking at. The cursor blink drops its handle the same way.
#[must_use = "a view pulses only while it holds this"]
pub struct Pulsing {
    _alive: Rc<()>,
}

impl Pulsing {
    /// The step to draw the pulse at now, `0..PULSE_STEPS`, where 0 is full brightness and `PULSE_STEPS / 2` the
    /// dimmest; the same for every handle. Always 0 under reduced motion, so the pulse stands still there. The view is
    /// notified each time the answer changes. It says nothing about an inactive window, where no view holds a handle.
    pub fn step(&self, cx: &App) -> u64 {
        match installed::<Ticker>(cx) {
            Some(ticker) if !ticker.reduced_motion => pulse_step(ticker.ticks),
            _ => 0,
        }
    }
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

/// Record the operating system's reduced-motion setting. Under reduced motion views still draw their state, but no
/// animation ticks are delivered, so nothing moves on its own.
pub fn set_reduced_motion(reduced: bool, cx: &mut App) {
    if let Some(ticker) = installed_mut::<Ticker>(cx) {
        ticker.reduced_motion = reduced;
    }
}

/// Pin whether windows count as active (`Some`), or follow GPUI again (`None`). For runs where the platform cannot
/// report activity truthfully: headless Mutter, which reports every window inactive.
pub fn set_active_override(active: Option<bool>, cx: &mut App) {
    if let Some(ticker) = installed_mut::<Ticker>(cx) {
        ticker.active_override = active;
    }
}

/// Whether `window` counts as active for animation: what GPUI says, unless [`set_active_override`] pinned it. A view
/// that draws differently in an inactive window (a terminal's cursor, unfocused there) asks this rather than GPUI, so
/// it agrees with whether its ticks come.
pub fn is_active(window: &Window, cx: &App) -> bool {
    installed::<Ticker>(cx)
        .and_then(|ticker| ticker.active_override)
        .unwrap_or_else(|| window.is_window_active())
}

/// Start animating the view `cx` belongs to, in `window`: it is notified on every tick until the handle drops. Under
/// reduced motion the handle is still returned, but no tick loop starts.
pub fn animate<V: 'static>(window: &Window, cx: &mut Context<V>) -> Animating {
    Animating {
        _alive: subscribe(window, cx, Cadence::Every(1)).0,
    }
}

/// Start the cursor blink of the view `cx` belongs to, in `window`, on its shown half: the view is notified each time
/// the blink changes half ([`BLINK_HALF_TICKS`]) until the handle drops, and reads the half from the handle. Under
/// reduced motion nothing ticks and the blink stays shown.
pub fn blink<V: 'static>(window: &Window, cx: &mut Context<V>) -> Blinking {
    let (alive, since) = subscribe(window, cx, Cadence::Every(BLINK_HALF_TICKS));
    Blinking {
        _alive: alive,
        since,
    }
}

/// Start the running pulse of the view `cx` belongs to, in `window`, at the shared phase's current step: the view is
/// notified each time the step changes until the handle drops, and reads the step from the handle. Under reduced
/// motion nothing ticks and the step stays 0.
pub fn pulse<V: 'static>(window: &Window, cx: &mut Context<V>) -> Pulsing {
    Pulsing {
        _alive: subscribe(window, cx, Cadence::PulseStep).0,
    }
}

/// Subscribe the view `cx` belongs to, notified on the ticks `cadence` picks, and start the tick loop unless it runs or
/// motion is reduced. Returns the handle's liveness token and the tick count it started at.
fn subscribe<V: 'static>(window: &Window, cx: &mut Context<V>, cadence: Cadence) -> (Rc<()>, u64) {
    let alive = Rc::new(());
    let view = cx.entity_id();
    let Some(ticker) = installed_mut::<Ticker>(cx) else {
        return (alive, 0);
    };
    let since = ticker.ticks;
    ticker.animating.push(Subscriber {
        alive: Rc::downgrade(&alive),
        view,
        window: window.window_handle(),
        cadence,
        since,
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
        if let Some(ticker) = installed_mut::<Ticker>(cx) {
            ticker.tick_loop = Some(task);
        }
    }
    (alive, since)
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

/// One tick: count it, and notify every live subscriber that is due and whose window is active. Returns whether the loop
/// should keep running: not once nobody animates, and not under reduced motion.
fn tick(cx: &mut App) -> bool {
    let (subscribers, override_active) = {
        let Some(ticker) = installed_mut::<Ticker>(cx) else {
            return false;
        };
        ticker
            .animating
            .retain(|subscriber| subscriber.alive.strong_count() > 0);
        if ticker.animating.is_empty() || ticker.reduced_motion {
            ticker.running = false;
            return false;
        }
        ticker.ticks += 1;
        let now = ticker.ticks;
        let subscribers: Vec<(EntityId, AnyWindowHandle)> = ticker
            .animating
            .iter()
            .filter(|subscriber| subscriber.cadence.due(now, subscriber.since))
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

/// How many ticks have been delivered since the app started: a test's evidence that the tick loop, and with it the
/// executor timer, ran or stood still over some stretch of the executor's clock.
#[cfg(test)]
pub(crate) fn ticks(cx: &App) -> u64 {
    installed::<Ticker>(cx).map_or(0, |ticker| ticker.ticks)
}

/// The crate's one timer: everything time-based goes through [`after`] or the tick loop that [`animate`], [`blink`] and
/// [`pulse`] start, which call this.
#[allow(
    clippy::disallowed_methods,
    reason = "the ticker is the one owner of GUI time (SPEC_impl.md, \"Animation time: the ticker\")"
)]
fn timer(executor: &gpui::BackgroundExecutor, duration: Duration) -> gpui::Task<()> {
    executor.timer(duration)
}
