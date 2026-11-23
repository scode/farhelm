//! The ticker: the one source of time for the GUI, for animation and for deadlines.
//!
//! SPEC_impl.md, "Animation time: the ticker". It serves these kinds of time:
//!
//! - Periodic animation ticks, at most [`TICKS_PER_SECOND`] a second, only while some view holds an [`Animating`],
//!   [`Blinking`] or [`Pulsing`] handle, only for views whose window is active, and never under reduced motion. A view
//!   that animates holds a handle and is notified on the ticks it needs; it never runs a timer of its own. Without one
//!   shared clock, a single animating row redraws its whole list at the display's rate (Zed's own agent sidebar pins a
//!   CPU core that way), and the old app had the same "nothing redraws on its own more than 10 times a second" rule.
//! - One-shot deadlines ([`after`]): a hover delay, a banner's expiry, a "still waiting" threshold. They fire on time
//!   whether or not the window is active, because they are about the user's time, not about drawing.
//! - Wall-clock time, for text that says how long ago something happened (a session row's age, the details bar's, the
//!   stopped card's). [`wall_now`] reads the time of day, and a view that shows such text holds a [`Clock`] from
//!   [`clock`], which notifies it every [`CLOCK_PERIOD`] (the old app re-rendered its relative times every 30 seconds)
//!   whether or not its window is active and whatever the reduced-motion setting. An age is information, not motion,
//!   and an inactive window is often still on screen, where a frozen "2m" would be wrong; one redraw every 30 seconds
//!   costs nothing worth saving. One loop serves every such [`Clock`] in the app, so a list of 200 rows that each show
//!   an age is one view redrawn twice a minute, not 200 timers.
//! - Counts of whole seconds, for text that says how long something has been going on (how long a Restart has waited,
//!   a host setup's elapsed time). A view that shows one holds a [`Clock`] from [`every_second`] instead of from
//!   [`clock`], and is notified every [`SECONDS_PERIOD`] on the same terms as an age: in an inactive window and under
//!   reduced motion too, because a count is information, not motion, and one that froze while the window was in the
//!   background would be wrong when the user looks. Its holders are few and short-lived (a count shows only while
//!   something is in progress), so one wake-up a second while one shows costs little, and one loop serves them all.
//!   The one-second clock has its own loop beside the 30-second one, so a view that shows only ages is never woken
//!   every second.
//!
//! GUI crates may not read the system clock or start timers (`crates/gui/clippy.toml`), so tests drive all of this
//! through GPUI's executor clock, and this module holds the crate's one exemption from the timer ban. The wall clock
//! needs no exemption: [`wall_now`] reads the time of day from GPUI's scheduler, which reads the system clock in the
//! app and a simulated one in tests that moves only when the test advances it, and no GUI code calls
//! `SystemTime::now` at all.
//!
//! Whether a window counts as active comes from GPUI, unless [`set_active_override`] says otherwise: headless Mutter
//! reports every window inactive forever (SPEC_impl.md, "GPUI cookbook: traps"), so the test-automation bridge's runs
//! pin it. In-process tests do not need the override: GPUI's test platform reports activity, and a test deactivates a
//! window with `VisualTestContext::deactivate_window`. GPUI Kit feeds the operating system's reduced-motion setting
//! into GPUI. When that setting changes, the ticker stops or restarts its animation loop. Clocks and deadlines keep
//! their own timers because they track elapsed time.
//!
//! One-shot transitions are not the ticker's: an element that fades in as it appears goes through the component layer's
//! `components::entrance` (its module docs say why ticks cannot draw one). GPUI's own animation element reads the same
//! reduced-motion flag for those, so the entrance does not ask here; motion GPUI does not drive, such as the permission
//! control's fading outline (`components::outline_fade`), asks [`reduced_motion`].
//!
//! The ticks are counted, and an animation whose phase changes more slowly than every tick says how often it needs to
//! hear about it. A cursor blink ([`blink`]) changes twice a second, so its view is notified twice a second rather than
//! ten times, and asks its [`Blinking`] handle which half of the blink it is in; each blink starts on its own shown
//! half. The running pulse ([`pulse`]) is the other phase, and it is shared: every [`Pulsing`] handle reads the same
//! step of one cycle counted from the tick count, so every pulsing dot in the app steps together. A new phase goes here
//! too, beside the tick that drives it.
//!
//! Animations that share a tick share a frame, so the cadences are laid out to land on the same ticks. A cadence of
//! every N ticks counts from the next multiple of N at or after the tick it started on, not from that tick itself: the
//! blink then changes half only on multiples of [`BLINK_HALF_TICKS`], which are all pulse steps, and a window that both
//! blinks and pulses draws four frames a second instead of six. Counted from its own start, a blink that started on
//! any tick but 0 or 3 within the pulse's five-tick pattern fell between the pulse's steps, and the window drew about
//! 1.5 times the idle frames; since focus moves on clicks, that was the usual case (the terminal area's TE-P2
//! measurement, 2026-11-16).
//!
//! Every handle (an animation's, a blink's, a pulse's, either clock's [`Clock`]) works the same way: the view keeps it
//! as a field, the ticker keeps only a weak reference to it, and dropping it, or the view that holds it, ends that
//! view's notifications. A loop finds out at its next wake-up that nobody is left and stops there, so no timer outlives
//! the last handle by more than one period.

use std::rc::{Rc, Weak};
use std::time::{Duration, SystemTime};

use gpui::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, EntityId, Global, Subscription, Task,
    Window,
};

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

/// How often a view holding a [`Clock`] is notified: the old app re-rendered its relative times ("2m", "1h") every 30
/// seconds, so an age is at most half a minute stale.
pub const CLOCK_PERIOD: Duration = Duration::from_secs(30);

/// How often a view holding a [`Clock`] from [`every_second`] is notified: a count of whole seconds moves on once a
/// second.
pub const SECONDS_PERIOD: Duration = Duration::from_secs(1);

/// The ticker's state: who animates and who shows wall-clock text, and the loop that serves each while anyone does.
#[derive(Default)]
struct Ticker {
    animating: Vec<Subscriber>,
    /// The tick loop. It is cancelled when motion is reduced, or finishes at its next wake once nobody animates.
    tick_loop: Option<Task<()>>,
    running: bool,
    /// Keep the motion observer registered for the life of this global, including when the loop is paused.
    _motion_observer: Option<Subscription>,
    /// The 30-second clock that [`clock`] hands out, for ages.
    ages: ClockLoop,
    /// The one-second clock that [`every_second`] hands out, for counts of seconds.
    seconds: ClockLoop,
    active_override: Option<bool>,
    /// How many ticks have been delivered since the app started. Only differences between two readings mean anything;
    /// the count stands still while nothing animates.
    ticks: u64,
}

impl Global for Ticker {}

impl Ticker {
    /// The holders and loop of `period`'s clock.
    fn clock_loop(&mut self, period: Period) -> &mut ClockLoop {
        match period {
            Period::Ages => &mut self.ages,
            Period::Seconds => &mut self.seconds,
        }
    }
}

/// One view that animates: it is notified on the ticks its [`Cadence`] picks, while its window is active and its
/// handle lives.
struct Subscriber {
    alive: Weak<()>,
    view: EntityId,
    window: AnyWindowHandle,
    cadence: Cadence,
    /// The tick count [`Cadence::Every`] counts from: [`Cadence::start`] of the count when the subscriber started, which
    /// may still lie ahead of the count.
    since: u64,
}

/// One clock's holders, and the loop that notifies them every period while any holder lives. The loop stops by itself
/// once none of this clock's [`Clock`]s lives, and the next handle starts a new one, replacing (and so dropping) the
/// finished task, as with `tick_loop`. A clock's loop is separate from the tick loop because it answers to neither
/// window activity nor reduced motion, and because folding it into the ticks would keep a timer firing ten times a
/// second for a view that changes once a second or twice a minute.
#[derive(Default)]
struct ClockLoop {
    holders: Vec<ClockSubscriber>,
    task: Option<Task<()>>,
    running: bool,
}

/// One view that shows an age or a count of seconds: it is notified every period of its clock while its [`Clock`]
/// lives.
struct ClockSubscriber {
    alive: Weak<()>,
    view: EntityId,
}

/// Which of the ticker's two clocks a [`Clock`] belongs to: the ages' 30-second one or the counts' one-second one.
#[derive(Clone, Copy)]
enum Period {
    Ages,
    Seconds,
}

impl Period {
    /// The time between two notifications of this clock.
    fn duration(self) -> Duration {
        match self {
            Period::Ages => CLOCK_PERIOD,
            Period::Seconds => SECONDS_PERIOD,
        }
    }
}

/// Which ticks a subscriber hears about.
#[derive(Clone, Copy)]
enum Cadence {
    /// Every this many ticks, on the multiples of it: a subscriber's phase is counted from the first multiple at or
    /// after the tick it started on ([`Cadence::start`]), so every subscriber of one cadence is notified on the same
    /// ticks.
    Every(u64),
    /// Each tick on which the shared pulse phase ([`pulse_step`]) moves to its next step, whenever the subscriber
    /// started: the phase is everyone's.
    PulseStep,
}

impl Cadence {
    /// The tick count a subscriber that starts at tick count `ticks` counts its phase from: for [`Cadence::Every`], the
    /// next multiple of its period at or after `ticks` (the module docs say why), and `ticks` itself for the pulse,
    /// whose phase comes from the count alone.
    ///
    /// The result can lie up to a period minus one tick ahead of `ticks`. Until the count reaches it the subscriber
    /// stays in its first phase (a blink in its shown half) and is not notified.
    fn start(self, ticks: u64) -> u64 {
        match self {
            Cadence::Every(every) => ticks.next_multiple_of(every),
            Cadence::PulseStep => ticks,
        }
    }

    /// Whether tick number `now` (at least 1: the count moves before anyone is notified) is one this cadence is
    /// notified on, for a subscriber whose phase counts from tick count `since` ([`Cadence::start`]).
    ///
    /// An every-N subscriber is first notified a whole period after `since`, never at `since` itself: its phase does
    /// not change there (a blink is shown both before and at `since`), so a notification there would only redraw its
    /// window for nothing.
    fn due(self, now: u64, since: u64) -> bool {
        match self {
            Cadence::Every(every) => now > since && (now - since).is_multiple_of(every),
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

/// A view's own elapsed animation ticks. The counter gets the ordinary
/// animation notifications, so it pauses under reduced motion and when the
/// window goes inactive. Dropping it ends its ticks and redraws.
pub(crate) struct AnimationTicks {
    counter: Entity<TickCounter>,
    _redraw: Subscription,
}

/// Use a separate entity so the owner's unrelated redraws cannot advance the animation.
struct TickCounter {
    count: u64,
    _animating: Animating,
    _ticks: Subscription,
}

impl AnimationTicks {
    /// Start at zero and notify the owner on each delivered animation tick.
    pub(crate) fn start<V: 'static>(window: &Window, cx: &mut Context<V>) -> Self {
        let counter = cx.new(|cx| TickCounter {
            count: 0,
            _animating: animate(window, cx),
            _ticks: cx.observe_self(|counter: &mut TickCounter, _| {
                counter.count = counter.count.wrapping_add(1);
            }),
        });
        Self {
            _redraw: cx.observe(&counter, |_, _, cx| cx.notify()),
            counter,
        }
    }

    /// Ticks delivered since this animation started, independent of other animations in the window.
    pub(crate) fn count(&self, cx: &App) -> u64 {
        self.counter.read(cx).count
    }
}

/// Held by a view whose cursor blinks, for as long as it blinks. Dropping it stops the view's blink ticks; making a new
/// one starts the blink over on its shown half, which is how a terminal that takes focus shows its cursor at once.
///
/// That first shown half lasts from half a second up to 0.9 s, not exactly half a second: the blink's halves change
/// only on multiples of [`BLINK_HALF_TICKS`], so that they land on the pulse's frames (the module docs say why), and
/// the first half also covers the ticks up to the first such multiple.
#[must_use = "a view blinks only while it holds this"]
pub struct Blinking {
    _alive: Rc<()>,
    /// The tick count the halves count from, the same as the subscriber's: [`Cadence::start`] of the count when the
    /// blink started, so it may lie ahead of the count while the first shown half lasts.
    since: u64,
}

impl Blinking {
    /// Whether the blink is in its shown half now. Always shown under reduced motion, so a cursor never stays hidden
    /// because the blink stopped in its hidden half. The view is notified each time this changes, while its window is
    /// active; in an inactive window the answer may change unannounced, so a view there should not draw from it.
    pub fn is_on(&self, cx: &App) -> bool {
        match installed::<Ticker>(cx) {
            Some(ticker) if !reduced_motion(cx) => {
                // Saturating, because `since` lies ahead of the count until the first multiple of the half comes: the
                // ticks before it are part of the first shown half.
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
            Some(ticker) if !reduced_motion(cx) => pulse_step(ticker.ticks),
            _ => 0,
        }
    }
}

/// Held by a view that shows wall-clock text (an age) or a count of seconds, for as long as it shows some. Dropping it,
/// or the view, stops the view's clock notifications.
///
/// A handle from [`clock`] is notified every [`CLOCK_PERIOD`], one from [`every_second`] every [`SECONDS_PERIOD`], in
/// an inactive window and under reduced motion too. The view reads the time to draw from [`wall_now`] when it renders;
/// the handle carries no time of its own. One handle serves everything the view draws, so a list keeps one for all its
/// rows: a row that is not a view of its own holds none, and nothing runs a timer per row. While a view holds the
/// one-second clock its ages stay fresh too, but a view that also shows ages keeps its 30-second handle, since it holds
/// the one-second one only while a count shows and its ages would freeze once the count goes. Every handle of a clock
/// shares that clock's one cadence, so a new handle's first notification may come sooner than a whole period after it
/// was made, and a count of seconds turns over up to a second after the moment it counts from does. A view that draws
/// an age or a count is drawn uncached: the cache wrapper cannot see the time, so a cached row would replay
/// its old text however often its list is notified (`cache.rs` has the note).
#[must_use = "a view hears the clock only while it holds this"]
pub struct Clock {
    _alive: Rc<()>,
}

/// A one-shot deadline. Dropping it cancels the callback, which is how a hover that ends cancels its tooltip.
#[must_use = "dropping a deadline cancels it"]
pub struct Deadline {
    _task: Task<()>,
}

/// Install the ticker and follow changes to the preference Kit applies during bootstrap.
pub fn install(cx: &mut App) {
    let motion_observer = cx.observe_reduce_motion(motion_changed);
    cx.set_global(Ticker {
        _motion_observer: Some(motion_observer),
        ..Ticker::default()
    });
}

/// Tests set the preference directly because Kit skips platform queries under the simulated scheduler.
#[cfg(test)]
pub(crate) fn set_reduced_motion(reduced: bool, cx: &mut App) {
    cx.set_reduce_motion(reduced);
}

/// The same live preference GPUI and Kit use, for motion neither the ticks nor GPUI's animation element drive: the
/// permission control's fading outline, for example, shows its final state at once under reduced motion.
pub fn reduced_motion(cx: &App) -> bool {
    cx.reduce_motion()
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
/// motion is reduced. Returns the handle's liveness token and the tick count its phase counts from
/// ([`Cadence::start`]), which a handle that reads its own phase ([`Blinking`]) must count from too, or it would change
/// phase on ticks its view is not notified on.
fn subscribe<V: 'static>(window: &Window, cx: &mut Context<V>, cadence: Cadence) -> (Rc<()>, u64) {
    let alive = Rc::new(());
    let view = cx.entity_id();
    let Some(ticker) = installed_mut::<Ticker>(cx) else {
        return (alive, 0);
    };
    let since = cadence.start(ticker.ticks);
    ticker.animating.push(Subscriber {
        alive: Rc::downgrade(&alive),
        view,
        window: window.window_handle(),
        cadence,
        since,
    });
    start_tick_loop(cx);
    (alive, since)
}

/// Stop the animation timer at the preference change. Keep the holders so a cursor or pulse can resume on the
/// same handle. While paused, nothing prunes dead holders until motion returns.
fn motion_changed(cx: &mut App) {
    let reduced = reduced_motion(cx);
    let Some(ticker) = installed_mut::<Ticker>(cx) else {
        return;
    };
    if reduced {
        ticker.tick_loop = None;
        ticker.running = false;
        return;
    }
    ticker
        .animating
        .retain(|subscriber| subscriber.alive.strong_count() > 0);
    start_tick_loop(cx);
}

/// Start one loop for the registered animations when motion is allowed and a holder needs it.
fn start_tick_loop(cx: &mut App) {
    if reduced_motion(cx) {
        return;
    }
    let Some(ticker) = installed_mut::<Ticker>(cx) else {
        return;
    };
    if ticker.running || ticker.animating.is_empty() {
        return;
    }
    ticker.running = true;
    let task = periodic(TICK, tick, cx);
    if let Some(ticker) = installed_mut::<Ticker>(cx) {
        ticker.tick_loop = Some(task);
    }
}

/// Start notifying the view `cx` belongs to every [`CLOCK_PERIOD`] until the handle drops, whatever its window's
/// activity and the reduced-motion setting: for a view that shows an age, which it reads from [`wall_now`] as it
/// renders.
///
/// Unlike [`animate`], this takes no window: whether the window is active never matters to it.
pub fn clock<V: 'static>(cx: &mut Context<V>) -> Clock {
    hold(Period::Ages, cx)
}

/// Start notifying the view `cx` belongs to every [`SECONDS_PERIOD`] until the handle drops, on the same terms as
/// [`clock`]: for a view that shows a count of whole seconds, which it works out from [`wall_now`] as it renders. A
/// view holds it only while it shows a count, since every holder keeps the one-second loop's timer firing. A count that
/// first shows after an exact threshold (the sidebar's waiting line, two seconds into a request) keeps an [`after`]
/// deadline for the threshold and takes this handle when the deadline fires: the shared cadence would otherwise show it
/// up to a second late, and a reply in that second would never show a line at all.
pub fn every_second<V: 'static>(cx: &mut Context<V>) -> Clock {
    hold(Period::Seconds, cx)
}

/// Add the view `cx` belongs to as a holder of `period`'s clock, and start that clock's loop unless it runs.
fn hold<V: 'static>(period: Period, cx: &mut Context<V>) -> Clock {
    let alive = Rc::new(());
    let view = cx.entity_id();
    let Some(ticker) = installed_mut::<Ticker>(cx) else {
        return Clock { _alive: alive };
    };
    let clock = ticker.clock_loop(period);
    clock.holders.push(ClockSubscriber {
        alive: Rc::downgrade(&alive),
        view,
    });
    if !clock.running {
        clock.running = true;
        let task = periodic(period.duration(), move |cx| clock_tick(period, cx), cx);
        if let Some(ticker) = installed_mut::<Ticker>(cx) {
            ticker.clock_loop(period).task = Some(task);
        }
    }
    Clock { _alive: alive }
}

/// The time of day now, for text that says how long ago something happened: the system clock in the app, and in tests
/// GPUI's simulated clock, which starts at a fixed instant and moves only as the test advances it, so an age a test
/// reads is exact.
///
/// It is wall-clock time, not a monotonic one: it can jump when the system clock is set, and it keeps counting while
/// the machine sleeps, which is what an age shown beside a session needs. A view that draws from it must hold a
/// [`Clock`], or its text goes stale until something else redraws it.
pub fn wall_now(cx: &App) -> SystemTime {
    // The scheduler's clock is the one GPUI's tests control (its `utc_now` advances with `advance_clock`); in the app
    // it is chrono's reading of the system clock. Converting from chrono's type names no chrono item, so this crate
    // takes no dependency for it.
    let utc = cx
        .background_executor()
        .scheduler_executor()
        .scheduler()
        .clock()
        .utc_now();
    SystemTime::from(utc)
}

/// A task that waits `period` and runs `step`, again and again until `step` returns false: the shape the tick loop and
/// the clock loops share. Each loop's `step` decides when nobody needs it any more, so the loop, and with it its timer,
/// ends on the first wake-up after its last handle dropped.
fn periodic(period: Duration, step: impl Fn(&mut App) -> bool + 'static, cx: &mut App) -> Task<()> {
    cx.spawn(async move |cx| {
        loop {
            timer(cx.background_executor(), period).await;
            if !cx.update(&step) {
                break;
            }
        }
    })
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
    let reduced = reduced_motion(cx);
    let (subscribers, override_active) = {
        let Some(ticker) = installed_mut::<Ticker>(cx) else {
            return false;
        };
        ticker
            .animating
            .retain(|subscriber| subscriber.alive.strong_count() > 0);
        if ticker.animating.is_empty() || reduced {
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

/// One period of `period`'s clock: notify every view whose [`Clock`] of that clock still lives, active window or not,
/// reduced motion or not. Returns whether the loop should keep running: not once none of its [`Clock`]s lives.
fn clock_tick(period: Period, cx: &mut App) -> bool {
    let views: Vec<EntityId> = {
        let Some(ticker) = installed_mut::<Ticker>(cx) else {
            return false;
        };
        let clock = ticker.clock_loop(period);
        clock
            .holders
            .retain(|subscriber| subscriber.alive.strong_count() > 0);
        if clock.holders.is_empty() {
            clock.running = false;
            return false;
        }
        clock
            .holders
            .iter()
            .map(|subscriber| subscriber.view)
            .collect()
    };
    for view in views {
        cx.notify(view);
    }
    true
}

/// How many ticks have been delivered since the app started: a test's evidence that the tick loop, and with it the
/// executor timer, ran or stood still over some stretch of the executor's clock.
#[cfg(test)]
pub(crate) fn ticks(cx: &App) -> u64 {
    installed::<Ticker>(cx).map_or(0, |ticker| ticker.ticks)
}

/// Test whether the animation task was released at the preference change, before its next scheduled tick.
#[cfg(test)]
pub(crate) fn holds_animation_task(cx: &App) -> bool {
    installed::<Ticker>(cx).is_some_and(|ticker| ticker.tick_loop.is_some())
}

/// The crate's one timer: everything time-based goes through [`after`], the tick loop that [`animate`], [`blink`] and
/// [`pulse`] start, or the clock loops that [`clock`] and [`every_second`] start, which call this.
#[allow(
    clippy::disallowed_methods,
    reason = "the ticker is the one owner of GUI time (SPEC_impl.md, \"Animation time: the ticker\")"
)]
fn timer(executor: &gpui::BackgroundExecutor, duration: Duration) -> gpui::Task<()> {
    executor.timer(duration)
}
