//! Relative moves and positions for a complete stored manual order.
//!
//! Each table chooses its own key, tie break and SQL. The caller keeps the
//! order read and the resulting writes in one transaction so a request can
//! rely on the neighbor it found even when the visible list was filtered.

use anyhow::Context;

/// Move an existing entry relative to another, returning the missing key on
/// failure. A present self-move is a no-op. A distinct-key move still calls
/// for renumbering even if the entries already sort beside each other.
pub(super) fn place<'a, Key: Eq>(
    entries: &mut Vec<(Key, i64)>,
    moved: &'a Key,
    neighbor: &'a Key,
    before: bool,
) -> Result<bool, &'a Key> {
    let moved_index = entries
        .iter()
        .position(|(key, _)| key == moved)
        .ok_or(moved)?;
    let neighbor_index = entries
        .iter()
        .position(|(key, _)| key == neighbor)
        .ok_or(neighbor)?;
    if moved_index == neighbor_index {
        return Ok(false);
    }
    let entry = entries.remove(moved_index);
    let neighbor_index = neighbor_index - usize::from(moved_index < neighbor_index);
    entries.insert(neighbor_index + usize::from(!before), entry);
    Ok(true)
}

/// Assign contiguous positions to the complete order, writing only values
/// that changed. Keep the original positions on the entries so the caller
/// can update the fleet revision only when a stored value actually moved.
/// Positions must allow ties while the transaction rewrites them.
pub(super) fn renumber<Key>(
    entries: &[(Key, i64)],
    mut write: impl FnMut(&Key, i64) -> rusqlite::Result<usize>,
) -> anyhow::Result<bool> {
    let mut changed = false;
    for (index, (key, previous)) in entries.iter().enumerate() {
        let position = i64::try_from(index).context("too many entries to renumber")?;
        if position != *previous {
            changed |= write(key, position)? > 0;
        }
    }
    Ok(changed)
}

/// Allocate the next position. The expensive full-order read and rewrite is
/// reserved for overflow; `compact` returns the size of the compacted order.
pub(super) fn append_position(
    maximum: Option<i64>,
    compact: impl FnOnce() -> anyhow::Result<usize>,
) -> anyhow::Result<i64> {
    match maximum {
        None => Ok(0),
        Some(i64::MAX) => i64::try_from(compact()?).context("too many entries to append"),
        Some(position) => Ok(position + 1),
    }
}
