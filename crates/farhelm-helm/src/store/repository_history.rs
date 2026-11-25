//! Managed checkout suggestions remember accepted repository intent across host lifetimes.
//!
//! The create transaction establishes provenance and novelty. A separate bounded projection keeps ordinary
//! creates, installation adoption and disposable host removal from aging out unrelated repositories.

use std::collections::HashSet;

use super::{Context, HostId, MAX_LAUNCH_HISTORY, history_order_is_newer};
use farhelm_proto::{GithubRepo, parse_github_repo};
use rusqlite::Transaction;

pub(super) const SCHEMA: &str = "CREATE TABLE github_repository_history (
    use_order INTEGER PRIMARY KEY,
    github_repo TEXT NOT NULL UNIQUE
) STRICT;
CREATE TABLE github_repository_replacement_admissions (
    host_id INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
    host_identity TEXT NOT NULL,
    session_id TEXT NOT NULL,
    ordering_kind INTEGER NOT NULL,
    ordering_value INTEGER NOT NULL,
    PRIMARY KEY (host_id, host_identity, session_id)
) STRICT;";

/// Seed once from surviving history using the timestamps available before helm-wide ordering existed.
/// Prefer the helm's admission time for a launch retained in both tables, since the supervisor clock
/// can differ from the helm's. The remaining sort keys make ties repeatable across upgrades.
pub(super) fn migrate(tx: &Transaction<'_>) -> anyhow::Result<()> {
    // Some historical tests rewind the version stamp on a current fixture. Keep any global history
    // they leave in place. Real upgrades create and populate the table in one transaction.
    let already_created: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'github_repository_history')",
        [],
        |row| row.get(0),
    )?;
    if already_created {
        return Ok(());
    }
    tx.execute_batch(SCHEMA)?;
    let mut query = tx.prepare(
        "SELECT github_repo FROM (
             SELECT ordinary.github_repo, ordinary.created_at AS used_at, 0 AS source,
                    ordinary.rowid AS source_order
             FROM create_history_sessions AS ordinary JOIN hosts ON hosts.id = ordinary.host_id
             WHERE ordinary.github_repo IS NOT NULL AND NOT EXISTS (
                 SELECT 1 FROM environment_launch_history AS environment
                 WHERE environment.origin_entry_uuid = hosts.entry_uuid
                   AND environment.origin_host_identity = ordinary.host_identity
                   AND environment.session_id = ordinary.session_id
                   AND environment.github_repo = ordinary.github_repo COLLATE NOCASE
             )
             UNION ALL
             SELECT github_repo, admitted_at, 1, admission_order
             FROM environment_launch_history WHERE github_repo IS NOT NULL
         ) ORDER BY used_at DESC, source DESC, source_order DESC",
    )?;
    let mut rows = query.query([])?;
    let mut seen = HashSet::new();
    let mut newest_first = Vec::new();
    while let Some(row) = rows.next()? {
        let raw: String = row.get(0)?;
        let Ok(GithubRepo { owner, name }) = parse_github_repo(&raw) else {
            continue;
        };
        let canonical = format!("{owner}/{name}");
        if seen.insert(canonical.clone()) {
            newest_first.push(canonical);
            if newest_first.len() == MAX_LAUNCH_HISTORY as usize {
                break;
            }
        }
    }
    drop(rows);
    let mut insert =
        tx.prepare("INSERT INTO github_repository_history (github_repo) VALUES (?1)")?;
    for repo in newest_first.into_iter().rev() {
        insert.execute([repo])?;
    }
    Ok(())
}

/// Promote a validated repository in helm admission order, which is shared across all supervisors. SQLite
/// allocates a fresh rowid for each insert attempt, including the upsert that moves an existing repository.
pub(super) fn record(tx: &Transaction<'_>, repo: &str) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO github_repository_history (github_repo) VALUES (?1)
         ON CONFLICT (github_repo) DO UPDATE SET use_order = excluded.use_order",
        [repo],
    )
    .context("recording a managed checkout repository")?;
    tx.execute(
        "DELETE FROM github_repository_history WHERE use_order IN (
             SELECT use_order FROM github_repository_history ORDER BY use_order DESC LIMIT -1 OFFSET ?1
         )",
        [MAX_LAUNCH_HISTORY],
    )
    .context("evicting old managed checkout repositories")?;
    Ok(())
}

/// A replacement in an existing disposable environment must not age its ordinary launcher history.
/// Keep its bounded replay window here. The installation's creation sequence supplies the ordering for
/// current peers. Observations without a sequence can be compared with each other by creation time.
pub(super) fn record_replacement(
    tx: &Transaction<'_>,
    host: HostId,
    identity: &str,
    session: &farhelm_proto::SessionInfo,
    sequence: Option<i64>,
    repo: &str,
) -> anyhow::Result<bool> {
    use rusqlite::{OptionalExtension, params};

    let ordering_kind = i64::from(sequence.is_some());
    let ordering_value = sequence.unwrap_or(session.created_at);
    let boundary: Option<(i64, i64, String)> = tx
        .query_row(
            "SELECT ordering_kind, ordering_value, session_id
         FROM github_repository_replacement_admissions WHERE host_id = ?1 AND host_identity = ?2
         ORDER BY ordering_kind DESC, ordering_value DESC, session_id ASC LIMIT 1 OFFSET ?3",
            params![host, identity, MAX_LAUNCH_HISTORY - 1],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if boundary.is_some_and(|(kind, value, session_id)| {
        !history_order_is_newer(
            (ordering_kind, ordering_value, &session.id),
            (kind, value, &session_id),
        )
    }) {
        return Ok(false);
    }
    let admitted = tx.execute(
        "INSERT INTO github_repository_replacement_admissions
            (host_id, host_identity, session_id, ordering_kind, ordering_value)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (host_id, host_identity, session_id) DO NOTHING",
        params![host, identity, session.id, ordering_kind, ordering_value],
    )? != 0;
    if !admitted {
        return Ok(false);
    }
    tx.execute(
        "DELETE FROM github_repository_replacement_admissions WHERE rowid IN (
             SELECT rowid FROM github_repository_replacement_admissions
             WHERE host_id = ?1 AND host_identity = ?2
             ORDER BY ordering_kind DESC, ordering_value DESC, session_id ASC LIMIT -1 OFFSET ?3
         )",
        params![host, identity, MAX_LAUNCH_HISTORY],
    )?;
    record(tx, repo)?;
    Ok(true)
}

/// Read the same newest-first order for every destination and revalidate identifiers before returning them.
fn list(conn: &rusqlite::Connection) -> anyhow::Result<Vec<GithubRepo>> {
    let mut query = conn.prepare(
        "SELECT github_repo FROM github_repository_history ORDER BY use_order DESC LIMIT ?1",
    )?;
    let mut rows = query.query([MAX_LAUNCH_HISTORY])?;
    let mut repos = Vec::new();
    while let Some(row) = rows.next()? {
        let raw: String = row.get(0)?;
        if let Ok(repo) = parse_github_repo(&raw) {
            repos.push(repo);
        }
    }
    Ok(repos)
}

impl super::HelmStore {
    /// Read accepted repository intent independently of host-specific launch and folder history.
    pub(crate) async fn global_github_repository_history(&self) -> anyhow::Result<Vec<GithubRepo>> {
        self.conn
            .call(
                "global repository history task panicked",
                |conn: &mut rusqlite::Connection| list(conn),
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cap applies to distinct repositories. Using the oldest one again must save it from eviction, and
    /// inserting one more must remove the next oldest row from storage, not merely from the returned list.
    #[test]
    fn github_repo_history_evicts_the_oldest_distinct_repository() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        let tx = conn.transaction().unwrap();
        for index in 0..MAX_LAUNCH_HISTORY {
            record(&tx, &format!("org/repo-{index}")).unwrap();
        }
        record(&tx, "org/repo-0").unwrap();
        record(&tx, "org/new").unwrap();
        tx.commit().unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM github_repository_history",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, MAX_LAUNCH_HISTORY);
        let repos = list(&conn).unwrap();
        assert_eq!(repos[0].name, "new");
        assert_eq!(repos[1].name, "repo-0");
        assert!(repos.iter().all(|repo| repo.name != "repo-1"));
    }
}
