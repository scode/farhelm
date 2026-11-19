//! The desktop app's updater as the window sees it: whether one runs, the readout it last published, and the commands
//! the window sends it. The updater itself, with its schedule, its downloads and its installs, is the desktop crate's
//! (`farhelm_desktop::updater`); everything here crosses the bootstrap's `Host` as plain values and callbacks, because
//! the GUI crate runs no threads and reads no clock.
//!
//! There is at most one question every update surface asks: does an updater run? It runs only in the installed release
//! bundle on a Mac (the desktop's `updater::start` decides that), so a devbuild, an agent build and the Linux app
//! answer no, and show no update surface at all (SPEC.md, "Installation and updates": "Where no updater runs ... the
//! readout stays plain text"; Settings → App shows only "in a desktop app whose updater is active"). Nothing here knows
//! why no updater runs, so no platform or build check is repeated on this side.
//!
//! The entry points other areas call are the plain functions below: [`runs`] for whether to draw a control at all,
//! [`check_now`] for every on-demand route (the version readout, Settings → App, the `?` menu's Check for updates and
//! the local host row's Update), and [`automatic_updates`] with [`set_automatic_updates`] for Settings → App. The
//! version readout (`update_readout::view`) holds the [`entity`], reads its [`Updater::readout`] and redraws when it
//! notifies; what it shows for a while after a result (Up to date's
//! five seconds, a dismissed failure) is its own presentation state, timed by the ticker, never kept here.

use std::future::Future;
use std::pin::Pin;

use gpui::{App, AppContext as _, Entity, Global, Task};

use crate::update_readout::Readout;

/// The next readout the updater publishes: resolves when its state changes, with the readout for the state it has
/// then (latest wins: changes in between are not reported one by one), or with `None` once the updater has stopped
/// publishing, after which it is not called again. Called again only once the previous call's future has resolved: the
/// desktop's feed keeps one subscription between calls, and a call made while another waits, or after one was dropped
/// unfinished, finds it gone and ends the feed.
pub type NextReadout = Box<dyn FnMut() -> Pin<Box<dyn Future<Output = Option<Readout>>>>>;

/// What the desktop hands the window when it started an updater: its feed and its commands. Each command runs on the
/// main thread and must return at once; slow work belongs to the updater's own worker.
pub struct UpdaterHost {
    /// The readout the updater publishes when the window opens; the window shows it until [`Self::next_readout`]
    /// brings another.
    pub first_readout: Readout,
    /// The feed of later readouts.
    pub next_readout: NextReadout,
    /// Check for a newer release now, and install it when there is one (every on-demand route does both).
    pub check_now: Box<dyn Fn()>,
    /// Start reopening the app once it has quit, for Restart to update. `true` means the app should now quit; `false`
    /// means the restart could not start (the updater's readout then says why), and the app must stay open, since
    /// quitting would leave it closed.
    pub restart_to_update: Box<dyn Fn() -> bool>,
    /// Whether automatic updates are on, from the app's own state. Read once, when the updater is kept: only this
    /// window changes the setting (through [`Self::set_automatic_updates`]), so the kept value follows its changes.
    pub automatic_updates: Box<dyn Fn() -> bool>,
    /// Turn automatic updates on or off; a failure carries a short reason for the setting's control to show, and the
    /// setting then stays as it was.
    pub set_automatic_updates: Box<dyn Fn(bool) -> Result<(), String>>,
}

/// The running updater's state on the window side: the readout it published last and its commands, with the task that
/// follows its feed. Views observe the entity, which notifies on every new readout.
pub struct Updater {
    readout: Readout,
    /// The automatic-updates setting as last read or saved, so a view drawing the switch reads no file.
    automatic: bool,
    check_now: Box<dyn Fn()>,
    restart_to_update: Box<dyn Fn() -> bool>,
    set_automatic_updates: Box<dyn Fn(bool) -> Result<(), String>>,
    _feed: Task<()>,
}

impl Updater {
    /// The readout the updater published last.
    pub fn readout(&self) -> &Readout {
        &self.readout
    }
}

/// The updater entity, present only while an updater runs.
struct Kept(Entity<Updater>);

impl Global for Kept {}

/// Keep the updater the host started, if any, and follow its feed for the app's life. With `None` nothing is kept,
/// and every function below answers as if no updater runs. Called once, before the main window opens, so the window's
/// first frame already knows whether to offer anything update-related.
pub(crate) fn keep(host: Option<UpdaterHost>, cx: &mut App) {
    let Some(host) = host else {
        return;
    };
    let UpdaterHost {
        first_readout,
        mut next_readout,
        check_now,
        restart_to_update,
        automatic_updates,
        set_automatic_updates,
    } = host;
    let updater = cx.new(|cx| Updater {
        readout: first_readout,
        automatic: automatic_updates(),
        check_now,
        restart_to_update,
        set_automatic_updates,
        _feed: cx.spawn(async move |updater, cx| {
            while let Some(readout) = next_readout().await {
                // An updater state change that leaves the readout as it was (a record refresh, say) redraws nothing.
                let shown = updater.update(cx, |updater: &mut Updater, cx| {
                    if updater.readout != readout {
                        updater.readout = readout;
                        cx.notify();
                    }
                });
                if shown.is_err() {
                    break;
                }
            }
        }),
    });
    cx.set_global(Kept(updater));
}

/// Whether an updater runs: the one question every update surface asks before drawing anything.
pub fn runs(cx: &App) -> bool {
    cx.has_global::<Kept>()
}

/// The updater entity, for a view that redraws when the readout changes; `None` when no updater runs.
pub fn entity(cx: &App) -> Option<Entity<Updater>> {
    cx.try_global::<Kept>().map(|Kept(updater)| updater.clone())
}

/// The readout the updater published last; `None` when no updater runs, which the version readout shows as its plain
/// build text.
pub fn readout(cx: &App) -> Option<Readout> {
    entity(cx).map(|updater| updater.read(cx).readout.clone())
}

/// Check for a newer release now and install it if there is one. Does nothing when no updater runs (the controls that
/// call this are not drawn then).
pub fn check_now(cx: &App) {
    if let Some(updater) = entity(cx) {
        (updater.read(cx).check_now)();
    }
}

/// Restart to update: start reopening the app, then quit it the ordinary way. When the restart cannot start, the app
/// stays open and the updater's readout says why.
pub fn restart_to_update(cx: &mut App) {
    restart_with(|cx| cx.quit(), cx);
}

/// [`restart_to_update`] with the quit passed in, because GPUI's test platform ignores a real quit.
fn restart_with(quit: impl FnOnce(&mut App), cx: &mut App) {
    let started = entity(cx).is_some_and(|updater| (updater.read(cx).restart_to_update)());
    if started {
        quit(cx);
    }
}

/// Whether automatic updates are on; `None` when no updater runs, so Settings → App shows no switch. Cheap enough to
/// call while drawing: it reads the kept value, not the app's state file.
pub fn automatic_updates(cx: &App) -> Option<bool> {
    entity(cx).map(|updater| updater.read(cx).automatic)
}

/// Turn automatic updates on or off, returning a failure's short reason for the switch to show. A saved change is
/// kept and notifies the entity's observers; a failed one leaves the setting as it was. With no updater there is no
/// switch to change, which is an error rather than a silent success.
pub fn set_automatic_updates(on: bool, cx: &mut App) -> Result<(), String> {
    let Some(updater) = entity(cx) else {
        return Err("No updater runs in this app".to_string());
    };
    updater.update(cx, |updater, cx| {
        (updater.set_automatic_updates)(on)?;
        updater.automatic = on;
        cx.notify();
        Ok(())
    })
}

/// An updater entity that shows `readout` for good, for the screenshot goldens. A version readout scene draws several
/// readout states side by side, each over an updater of its own, while the app keeps at most one updater ([`keep`]
/// sets a global). This one is never kept: it follows no feed and its commands do nothing, so nothing a scene does
/// changes what it shows, and the plain functions above, which go through the kept updater, never reach it.
#[cfg(test)]
pub(crate) fn fixture(readout: Readout, cx: &mut App) -> Entity<Updater> {
    cx.new(|_| Updater {
        readout,
        automatic: false,
        check_now: Box::new(|| {}),
        restart_to_update: Box::new(|| false),
        set_automatic_updates: Box::new(|_| Err("a fixture updater saves nothing".to_string())),
        _feed: Task::ready(()),
    })
}

#[cfg(test)]
pub(crate) mod fake;
#[cfg(test)]
mod tests;
