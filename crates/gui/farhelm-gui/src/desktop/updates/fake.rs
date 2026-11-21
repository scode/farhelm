//! A fake updater host for tests: plain closures over a feed the test publishes into and a record of the commands it
//! received. Shared by the updater's own tests and the version readout's, so both drive the window side the same way.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::task::{Poll, Waker};

use super::UpdaterHost;
use crate::update_readout::{Activity, Readout};

/// A readout told apart from others by its hover text alone.
pub(crate) fn readout_saying(tooltip: &str) -> Readout {
    Readout {
        update_ready: false,
        needs_reinstall: false,
        withdrawn: false,
        tooltip: tooltip.to_string(),
        activity: Activity::Idle,
        notice_id: 0,
        user_update_ready: false,
    }
}

/// The fake updater's feed: the test publishes into it, and the window's next-readout future resolves with what was
/// published, the latest value winning as the desktop's watch channel does. `Some(None)` is the end of the feed.
#[derive(Default)]
pub(crate) struct Feed {
    published: RefCell<Option<Option<Readout>>>,
    waiting: RefCell<Option<Waker>>,
}

impl Feed {
    pub(crate) fn publish(&self, readout: Option<Readout>) {
        *self.published.borrow_mut() = Some(readout);
        if let Some(waker) = self.waiting.borrow_mut().take() {
            waker.wake();
        }
    }
}

/// What the fake updater's commands saw.
#[derive(Default)]
pub(crate) struct Seen {
    pub(crate) checks: Cell<u32>,
    pub(crate) restarts: Cell<u32>,
    pub(crate) automatic: Cell<bool>,
}

/// A fake updater host: its feed, its first readout "first", and commands that record into `seen`. A restart starts
/// when `restart_starts` is set; turning automatic updates off fails, so the failure path is visible.
pub(crate) fn fake_host(feed: &Rc<Feed>, seen: &Rc<Seen>, restart_starts: bool) -> UpdaterHost {
    let next = feed.clone();
    let (checks, restarts, read, set) = (seen.clone(), seen.clone(), seen.clone(), seen.clone());
    UpdaterHost {
        first_readout: readout_saying("first"),
        next_readout: Box::new(move || {
            let feed = next.clone();
            Box::pin(std::future::poll_fn(move |cx| {
                match feed.published.borrow_mut().take() {
                    Some(published) => Poll::Ready(published),
                    None => {
                        *feed.waiting.borrow_mut() = Some(cx.waker().clone());
                        Poll::Pending
                    }
                }
            }))
        }),
        check_now: Box::new(move || checks.checks.set(checks.checks.get() + 1)),
        restart_to_update: Box::new(move || {
            restarts.restarts.set(restarts.restarts.get() + 1);
            restart_starts
        }),
        automatic_updates: Box::new(move || read.automatic.get()),
        set_automatic_updates: Box::new(move |on| {
            if on {
                set.automatic.set(true);
                Ok(())
            } else {
                Err("not saved".to_string())
            }
        }),
        upgrade_notes: None,
    }
}
