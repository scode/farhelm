//! Groups are helm-owned viewer state. Membership uses the same paired
//! host/session key as pins, and grouping a pin never creates a second
//! position sequence.

use anyhow::Context;
use farhelm_proto::session_requests::{SessionGroupMembership, UpdateSessionGroup};
use farhelm_proto::ui_channel::SessionGroup;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use unicase::UniCase;

use super::{HelmStore, PinMoveError, SessionKey, append_existing_pin, has_pin, manual_order};

/// The sequence cannot reuse a deleted ID. A menu or queued collapse write
/// may still address the group after its last member leaves.
pub(super) const SESSION_GROUPS_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS session_groups (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        folded_name TEXT NOT NULL UNIQUE,
        position INTEGER NOT NULL,
        collapsed INTEGER NOT NULL CHECK (collapsed IN (0, 1))
    ) STRICT;
    CREATE TABLE IF NOT EXISTS session_group_members (
        host_id INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
        session_id TEXT NOT NULL,
        group_id INTEGER NOT NULL REFERENCES session_groups (id) ON DELETE CASCADE,
        PRIMARY KEY (host_id, session_id)
    ) STRICT;
    CREATE INDEX IF NOT EXISTS session_group_members_by_group
        ON session_group_members (group_id);";

/// Refusals the helm can explain without exposing database errors.
#[derive(Debug, thiserror::Error)]
pub(crate) enum GroupError {
    #[error("enter a group name")]
    EmptyName,
    #[error("a group name may be at most {GROUP_NAME_CAP} bytes")]
    LongName,
    #[error("the session group list is full")]
    RosterFull,
    #[error("a group already has that name")]
    DuplicateName,
    #[error("the session group no longer exists")]
    Missing,
    #[error("a new group needs at least one session")]
    EmptySelection,
}

/// Bound names stored in every session-list reply, including filtered ones.
const GROUP_NAME_CAP: usize = 256;

/// Reserve most of the UI channel's reply budget for the session rows. A
/// name byte may use six bytes in JSON, and 128 bytes covers the other
/// fields of an entry, its punctuation and its separator.
const ROSTER_WIRE_BUDGET: i64 = 4 * 1024 * 1024;
const ROSTER_ENTRY_OVERHEAD: i64 = 128;
const JSON_NAME_EXPANSION: i64 = 6;

/// Refuse growth before it could make every filtered list too large for the
/// UI channel. A rename that leaves the roster the same size needs no scan.
fn check_roster_growth(tx: &Transaction<'_>, addition: i64) -> anyhow::Result<()> {
    if addition > 0 {
        let occupied: i64 = tx.query_row(
            "SELECT COALESCE(SUM(?1 + ?2 * length(CAST(name AS BLOB))), 0) FROM session_groups",
            params![ROSTER_ENTRY_OVERHEAD, JSON_NAME_EXPANSION],
            |row| row.get(0),
        )?;
        if occupied.saturating_add(addition) > ROSTER_WIRE_BUDGET {
            return Err(GroupError::RosterFull.into());
        }
    }
    Ok(())
}

/// The group for a paired session, independent of the session cache.
pub(super) fn group_of(conn: &Connection, key: &SessionKey) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT group_id FROM session_group_members WHERE host_id = ?1 AND session_id = ?2",
        params![key.host, key.session_id],
        |row| row.get(0),
    )
    .optional()
}

/// Change membership alone. The caller owns any pin placement and empty
/// group cleanup so both changes can commit with the membership.
pub(super) fn assign(
    tx: &Transaction<'_>,
    key: &SessionKey,
    group: Option<i64>,
) -> rusqlite::Result<bool> {
    let changed = match group {
        Some(group) => tx.execute(
            "INSERT INTO session_group_members (host_id, session_id, group_id)
             VALUES (?1, ?2, ?3) ON CONFLICT (host_id, session_id) DO UPDATE
             SET group_id = excluded.group_id WHERE group_id != excluded.group_id",
            params![key.host, key.session_id, group],
        )?,
        None => tx.execute(
            "DELETE FROM session_group_members WHERE host_id = ?1 AND session_id = ?2",
            params![key.host, key.session_id],
        )?,
    };
    Ok(changed != 0)
}

/// Retire an empty group in the transaction that removed its final member.
/// A host cascade can empty several groups without affecting groups that
/// still contain sessions on a different host.
pub(super) fn prune_empty(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM session_groups WHERE NOT EXISTS (
            SELECT 1 FROM session_group_members WHERE group_id = session_groups.id
        )",
        [],
    )?;
    Ok(())
}

/// Copy the membership before the old session's delete can empty its group.
pub(super) fn inherit(
    tx: &Transaction<'_>,
    source: &SessionKey,
    replacement: &SessionKey,
) -> rusqlite::Result<bool> {
    Ok(tx.execute(
        "INSERT OR IGNORE INTO session_group_members (host_id, session_id, group_id)
         SELECT ?3, ?4, group_id FROM session_group_members WHERE host_id = ?1 AND session_id = ?2",
        params![
            source.host,
            source.session_id,
            replacement.host,
            replacement.session_id
        ],
    )? != 0)
}

/// One roster for the list and its move menus. Membership counts include
/// rows outside the selected host or status filter.
pub(super) fn roster(conn: &Connection) -> anyhow::Result<Vec<SessionGroup>> {
    let mut statement = conn.prepare(
        "SELECT id, name, position, collapsed,
            (SELECT COUNT(*) FROM session_group_members WHERE group_id = session_groups.id)
         FROM session_groups ORDER BY position, id",
    )?;
    statement
        .query_map([], |row| {
            Ok(SessionGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                position: row.get(2)?,
                collapsed: row.get(3)?,
                members: u64::try_from(row.get::<_, i64>(4)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
            })
        })?
        .collect::<Result<_, _>>()
        .context("reading the session group roster")
}

/// A group's display spelling and its Unicode case-folded uniqueness key.
/// Folding also combines case equivalents such as the two lowercase sigmas.
fn name_parts(raw: &str) -> Result<(&str, String), GroupError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(GroupError::EmptyName);
    }
    if name.len() > GROUP_NAME_CAP {
        return Err(GroupError::LongName);
    }
    Ok((name, UniCase::new(name).to_folded_case()))
}

/// Refuse duplicate names while the caller holds the write transaction.
/// A rename may change the casing of its own name.
fn check_name(tx: &Transaction<'_>, folded: &str, own: Option<i64>) -> anyhow::Result<()> {
    let id: Option<i64> = tx
        .query_row(
            "SELECT id FROM session_groups WHERE folded_name = ?1",
            [folded],
            |row| row.get(0),
        )
        .optional()?;
    if id.is_some() && id != own {
        return Err(GroupError::DuplicateName.into());
    }
    Ok(())
}

/// Group positions use ID as a total tie break, just as pin positions use
/// their paired key. Both use the same placement and renumbering rules.
fn ordered_groups(conn: &Connection) -> anyhow::Result<Vec<(i64, i64)>> {
    let mut statement =
        conn.prepare("SELECT id, position FROM session_groups ORDER BY position, id")?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()
        .context("reading the session group order")
}

/// Supply the group write to the shared position compaction.
fn renumber_groups(tx: &Transaction<'_>, groups: &[(i64, i64)]) -> anyhow::Result<bool> {
    let mut update = tx.prepare("UPDATE session_groups SET position = ?2 WHERE id = ?1")?;
    manual_order::renumber(groups, |id, position| update.execute(params![id, position]))
}

impl HelmStore {
    /// Create one group and attach the entire selection, or write nothing.
    /// Input order is retained when appending the selection's pins.
    pub(crate) async fn create_session_group(
        &self,
        name: String,
        keys: Vec<SessionKey>,
    ) -> anyhow::Result<i64> {
        if keys.is_empty() {
            return Err(GroupError::EmptySelection.into());
        }
        self.conn
            .call(
                "session group create panicked",
                move |conn: &mut Connection| {
                    let tx =
                        conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                    let (name, folded) = name_parts(&name)?;
                    check_name(&tx, &folded, None)?;
                    check_roster_growth(
                        &tx,
                        ROSTER_ENTRY_OVERHEAD + JSON_NAME_EXPANSION * name.len() as i64,
                    )?;
                    let maximum =
                        tx.query_row("SELECT MAX(position) FROM session_groups", [], |row| {
                            row.get(0)
                        })?;
                    let position = manual_order::append_position(maximum, || {
                        let groups = ordered_groups(&tx)?;
                        renumber_groups(&tx, &groups)?;
                        Ok(groups.len())
                    })?;
                    let id = tx.query_row(
                        "INSERT INTO session_groups (name, folded_name, position, collapsed)
                     VALUES (?1, ?2, ?3, 0) RETURNING id",
                        params![name, folded, position],
                        |row| row.get(0),
                    )?;
                    for key in keys {
                        if assign(&tx, &key, Some(id))? {
                            append_existing_pin(&tx, &key)?;
                        }
                    }
                    prune_empty(&tx)?;
                    tx.commit()?;
                    Ok(id)
                },
            )
            .await
    }

    /// A menu move only changes a session that is going to a different
    /// area. An already pinned row appends there without losing its pin.
    pub(crate) async fn set_session_group(
        &self,
        key: SessionKey,
        request: SessionGroupMembership,
    ) -> anyhow::Result<bool> {
        self.conn
            .call(
                "session group move panicked",
                move |conn: &mut Connection| {
                    let tx =
                        conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                    if let Some(from) = request.pin_source {
                        if !has_pin(&tx, &key)? {
                            return Err(PinMoveError::NotPinned(key).into());
                        }
                        if group_of(&tx, &key)? != from.group() {
                            return Err(PinMoveError::AreaChanged.into());
                        }
                    }
                    let group = request.target.group();
                    if let Some(id) = group {
                        group_attributes(&tx, id)?;
                    }
                    let changed = assign(&tx, &key, group)?;
                    if changed {
                        append_existing_pin(&tx, &key)?;
                        prune_empty(&tx)?;
                    }
                    tx.commit()?;
                    Ok(changed)
                },
            )
            .await
    }

    /// A duplicate collapse choice or unchanged name wakes no clients.
    pub(crate) async fn update_session_group(
        &self,
        id: i64,
        patch: UpdateSessionGroup,
    ) -> anyhow::Result<bool> {
        self.conn
            .call("session group update panicked", move |conn: &mut Connection| {
                let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let (previous_name, previous_collapsed) = group_attributes(&tx, id)?;
                let (name, folded) = name_parts(patch.name.as_deref().unwrap_or(&previous_name))?;
                let collapsed = patch.collapsed.unwrap_or(previous_collapsed);
                if name == previous_name && collapsed == previous_collapsed {
                    return Ok(false);
                }
                check_name(&tx, &folded, Some(id))?;
                check_roster_growth(&tx, JSON_NAME_EXPANSION * (name.len() as i64 - previous_name.len() as i64))?;
                tx.execute(
                    "UPDATE session_groups SET name = ?2, folded_name = ?3, collapsed = ?4 WHERE id = ?1",
                    params![id, name, folded, collapsed],
                )?;
                tx.commit()?;
                Ok(true)
            })
            .await
    }

    /// Relative order has the same filtered-neighbor semantics as pins.
    pub(crate) async fn move_session_group(
        &self,
        moved: i64,
        neighbor: i64,
        before: bool,
    ) -> anyhow::Result<bool> {
        self.conn
            .call(
                "session group order panicked",
                move |conn: &mut Connection| {
                    let tx = conn.transaction()?;
                    let mut groups = ordered_groups(&tx)?;
                    if !manual_order::place(&mut groups, &moved, &neighbor, before)
                        .map_err(|_| GroupError::Missing)?
                    {
                        return Ok(false);
                    }
                    let changed = renumber_groups(&tx, &groups)?;
                    tx.commit()?;
                    Ok(changed)
                },
            )
            .await
    }

    /// Drop the group, appending its pins to the global pins in their old
    /// relative order. Unpinned members need only the membership cascade.
    pub(crate) async fn ungroup_sessions(&self, group: i64) -> anyhow::Result<bool> {
        self.conn
            .call("ungroup panicked", move |conn: &mut Connection| {
                let tx = conn.transaction()?;
                let pins = {
                    let mut query = tx.prepare(
                        "SELECT p.host_id, p.session_id FROM session_pins AS p
                         JOIN session_group_members AS m USING (host_id, session_id)
                         WHERE m.group_id = ?1 ORDER BY p.position, p.host_id, p.session_id",
                    )?;
                    query
                        .query_map([group], |row| {
                            Ok(SessionKey::new(row.get(0)?, row.get::<_, String>(1)?))
                        })?
                        .collect::<Result<Vec<_>, _>>()?
                };
                for key in &pins {
                    append_existing_pin(&tx, key)?;
                }
                let changed = tx.execute("DELETE FROM session_groups WHERE id = ?1", [group])? != 0;
                tx.commit()?;
                Ok(changed)
            })
            .await
    }

    /// Confirmed session deletion removes membership without moving a pin
    /// that the caller is already removing independently.
    pub(crate) async fn forget_session_group(&self, key: SessionKey) -> anyhow::Result<()> {
        self.conn
            .call(
                "session group cleanup panicked",
                move |conn: &mut Connection| {
                    let tx = conn.transaction()?;
                    if assign(&tx, &key, None)? {
                        prune_empty(&tx)?;
                    }
                    tx.commit()?;
                    Ok(())
                },
            )
            .await
    }
}

/// Read current attributes or report a stale group ID before any write.
fn group_attributes(conn: &Connection, id: i64) -> anyhow::Result<(String, bool)> {
    let found = conn
        .query_row(
            "SELECT name, collapsed FROM session_groups WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    found.ok_or_else(|| GroupError::Missing.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Duplicate names follow case folding even for letters with several
    /// lowercase forms. Bound their size before storing them in the roster.
    #[test]
    fn group_names_trim_fold_and_bound_the_stored_name() {
        assert_eq!(name_parts("  ΑΣ  ").unwrap().0, "ΑΣ");
        assert_eq!(name_parts("ΑΣ").unwrap().1, name_parts("ασ").unwrap().1);
        assert!(matches!(name_parts(" \t "), Err(GroupError::EmptyName)));
        assert!(matches!(
            name_parts(&"a".repeat(GROUP_NAME_CAP + 1)),
            Err(GroupError::LongName)
        ));
    }
}
