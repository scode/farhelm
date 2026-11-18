//! A mute belongs to the session's registry entry, with the same lifetime as its pin.
//!
//! After an unmute the row can still hold the old sort key. The work-start
//! stamp observed at unmute tells the listing when it can use the live key
//! again, without writing during list reads.

use super::{HelmStore, SessionKey, cached_session_on_connection, write_seen_stamp};
use farhelm_proto::{SessionInfo, SessionMute};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;

/// The schema is shared by fresh creation and migration. A database rewound
/// by changing just its version still keeps its session choices.
pub(super) const SESSION_MUTES_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS session_mutes (
    host_id INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
    session_id TEXT NOT NULL,
    until_ms INTEGER,
    frozen_work_start INTEGER NOT NULL,
    work_start_at_unmute INTEGER,
    PRIMARY KEY (host_id, session_id)
) STRICT;";

/// The two stamps come from one observation. Seen state uses the activity
/// stamp and the order uses the start of the current burst.
#[derive(Clone, Copy)]
pub(crate) struct MuteObservation {
    work_start: i64,
    activity: i64,
}

impl From<&SessionInfo> for MuteObservation {
    fn from(info: &SessionInfo) -> Self {
        Self {
            work_start: info.effective_work_started_at(),
            activity: info.effective_activity(),
        }
    }
}

/// Identity-less hosts supply their in-memory observation. For durable
/// hosts, read the cache on the write connection, with no refresh between
/// choosing the stamps and committing the metadata transition.
fn current_observation(
    conn: &Connection,
    key: &SessionKey,
    live: Option<MuteObservation>,
) -> anyhow::Result<Option<MuteObservation>> {
    match live {
        Some(observed) => Ok(Some(observed)),
        None => Ok(
            cached_session_on_connection(conn, key.host, &key.session_id)?
                .as_ref()
                .map(MuteObservation::from),
        ),
    }
}

/// An active mute has no unmute marker. An ended mute retains only the
/// evidence needed to hold the row in its old group through the current burst.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MuteEntry {
    until: Option<i64>,
    frozen_work_start: i64,
    work_start_at_unmute: Option<i64>,
}

impl MuteEntry {
    /// A spent order hold carries no visible mute state.
    pub(crate) fn active(self) -> Option<SessionMute> {
        self.work_start_at_unmute
            .is_none()
            .then_some(SessionMute { until: self.until })
    }

    /// Use the held key until a new work start is observed. The listing owns
    /// the connected-Waiting exception because it also owns the active group.
    pub(crate) fn order_hold(self, work_start: i64) -> Option<i64> {
        self.work_start_at_unmute
            .is_none_or(|at_unmute| work_start <= at_unmute)
            .then_some(self.frozen_work_start)
    }
}

fn mute_entry(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<MuteEntry> {
    Ok(MuteEntry {
        until: row.get(offset)?,
        frozen_work_start: row.get(offset + 1)?,
        work_start_at_unmute: row.get(offset + 2)?,
    })
}

impl HelmStore {
    /// Join mute and order state by the same keys as pins and seen stamps.
    pub(crate) async fn session_mutes(
        &self,
        keys: &[SessionKey],
    ) -> anyhow::Result<HashMap<SessionKey, MuteEntry>> {
        self.read_keyed_viewer_state(
            keys,
            "mute read task panicked",
            "SELECT host_id, session_id, until_ms, frozen_work_start, work_start_at_unmute \
             FROM session_mutes",
            |row| mute_entry(row, 2),
        )
        .await
    }

    /// An already muted session keeps its deadline and order on retries.
    /// If an earlier mute still holds the displayed key, a new mute starts
    /// from that key so the list stays in place.
    pub(crate) async fn mute_session(
        &self,
        key: &SessionKey,
        until: Option<i64>,
        live: Option<MuteObservation>,
    ) -> anyhow::Result<Option<bool>> {
        let key = key.clone();
        self.conn
            .call("mute write task panicked", move |conn: &mut Connection| {
                let tx = conn.transaction()?;
                let Some(order) = current_observation(&tx, &key, live)? else {
                    return Ok(None);
                };
                let previous = tx.query_row(
                    "SELECT until_ms, frozen_work_start, work_start_at_unmute FROM session_mutes \
                     WHERE host_id = ?1 AND session_id = ?2",
                    params![key.host, key.session_id],
                    |row| mute_entry(row, 0),
                ).optional()?;
                if previous.and_then(MuteEntry::active).is_some() {
                    return Ok(Some(false));
                }
                let frozen = previous
                    .and_then(|entry| entry.order_hold(order.work_start))
                    .unwrap_or(order.work_start);
                let changed = tx.execute(
                    "INSERT INTO session_mutes (host_id, session_id, until_ms, frozen_work_start) \
                     VALUES (?1, ?2, ?3, ?4) \
                     ON CONFLICT (host_id, session_id) DO UPDATE SET \
                         until_ms = excluded.until_ms, \
                         frozen_work_start = excluded.frozen_work_start, work_start_at_unmute = NULL",
                    params![key.host, key.session_id, until, frozen],
                )? != 0;
                tx.commit()?;
                Ok(Some(changed))
            })
            .await
    }

    /// End one mute and advance the seen stamp in the same transaction.
    /// A timed end must match the deadline it sampled, so an intervening
    /// unmute followed by another mute keeps the newer choice. Without a
    /// readable observation (e.g. an identity-less host went away), the
    /// saved order key is the last evidence available and the seen stamp stays.
    pub(crate) async fn unmute_session(
        &self,
        key: &SessionKey,
        deadline: Option<i64>,
        live: Option<MuteObservation>,
    ) -> anyhow::Result<bool> {
        let key = key.clone();
        self.conn
            .call(
                "unmute write task panicked",
                move |conn: &mut Connection| {
                    let tx = conn.transaction()?;
                    let observation = current_observation(&tx, &key, live)?;
                    let changed = tx.execute(
                        "UPDATE session_mutes SET \
                         until_ms = NULL, work_start_at_unmute = COALESCE(?3, frozen_work_start) \
                     WHERE host_id = ?1 AND session_id = ?2 AND work_start_at_unmute IS NULL \
                         AND (?4 IS NULL OR until_ms = ?4)",
                        params![
                            key.host,
                            key.session_id,
                            observation.map(|o| o.work_start),
                            deadline
                        ],
                    )? != 0;
                    if changed && let Some(observation) = observation {
                        write_seen_stamp(&tx, &key, observation.activity)?;
                    }
                    tx.commit()?;
                    Ok(changed)
                },
            )
            .await
    }

    /// Return due mutes, with their exact deadlines for the end check.
    pub(crate) async fn due_mutes(&self, now: i64) -> anyhow::Result<Vec<(SessionKey, i64)>> {
        self.conn
            .call(
                "due mute read task panicked",
                move |conn: &mut Connection| {
                    let mut query = conn.prepare(
                        "SELECT host_id, session_id, until_ms FROM session_mutes \
                     WHERE until_ms <= ?1 AND work_start_at_unmute IS NULL",
                    )?;
                    Ok(query
                        .query_map([now], |row| {
                            Ok((
                                SessionKey::new(row.get(0)?, row.get::<_, String>(1)?),
                                row.get(2)?,
                            ))
                        })?
                        .collect::<Result<Vec<_>, _>>()?)
                },
            )
            .await
    }

    /// The expiry loop waits for the next deadline or a fleet revision.
    pub(crate) async fn next_mute_deadline(&self) -> anyhow::Result<Option<i64>> {
        self.conn
            .call(
                "mute deadline read task panicked",
                |conn: &mut Connection| {
                    Ok(conn.query_row(
                    "SELECT MIN(until_ms) FROM session_mutes WHERE work_start_at_unmute IS NULL",
                    [],
                    |row| row.get(0),
                )?)
                },
            )
            .await
    }

    /// Confirmed deletion drops both a current mute and any remaining order hold.
    pub(crate) async fn forget_muted_session(&self, key: &SessionKey) -> anyhow::Result<()> {
        let key = key.clone();
        self.conn
            .call(
                "mute cleanup task panicked",
                move |conn: &mut Connection| {
                    conn.execute(
                        "DELETE FROM session_mutes WHERE host_id = ?1 AND session_id = ?2",
                        params![key.host, key.session_id],
                    )?;
                    Ok(())
                },
            )
            .await
    }
}

/// Prepare the replacement before deleting the source. An expired hold is
/// tied to the source's work-start stamp. Only an active mute follows it.
pub(super) fn inherit_mute(
    conn: &Connection,
    source: &SessionKey,
    replacement: &SessionKey,
) -> anyhow::Result<bool> {
    Ok(conn.execute(
        "INSERT INTO session_mutes (host_id, session_id, until_ms, frozen_work_start) \
         SELECT ?3, ?4, until_ms, frozen_work_start FROM session_mutes \
         WHERE host_id = ?1 AND session_id = ?2 AND work_start_at_unmute IS NULL \
         ON CONFLICT (host_id, session_id) DO NOTHING",
        params![
            source.host,
            source.session_id,
            replacement.host,
            replacement.session_id
        ],
    )? != 0)
}
