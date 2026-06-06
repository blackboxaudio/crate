//! Hybrid logical clock (HLC).
//!
//! Format: `{wall_ms:016x}-{counter:08x}-{node:08x}` — fixed-width lowercase hex,
//! so lexicographic string order equals `(wall_ms, counter, node)` tuple order.
//! The empty string `""` is the "never stamped" sentinel and sorts below every
//! real HLC, so an unstamped local row always loses to any stamped remote row.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};

use crate::error::{CrateError, Result};

pub type NodeId = u32;

/// Wall-clock milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hlc {
    pub wall_ms: u64,
    pub counter: u32,
    pub node: NodeId,
}

impl Hlc {
    pub fn new(wall_ms: u64, counter: u32, node: NodeId) -> Self {
        Self {
            wall_ms,
            counter,
            node,
        }
    }

    /// Advance for a local event against the current wall clock.
    pub fn tick(&mut self, now_ms: u64) {
        if now_ms > self.wall_ms {
            self.wall_ms = now_ms;
            self.counter = 0;
        } else {
            self.counter = self.counter.saturating_add(1);
        }
    }

    /// Merge a received remote HLC into the local clock (standard HLC receive).
    pub fn receive(&mut self, remote: &Hlc, now_ms: u64) {
        let max_wall = self.wall_ms.max(remote.wall_ms).max(now_ms);
        let new_counter = if max_wall == self.wall_ms && max_wall == remote.wall_ms {
            self.counter.max(remote.counter) + 1
        } else if max_wall == self.wall_ms {
            self.counter + 1
        } else if max_wall == remote.wall_ms {
            remote.counter + 1
        } else {
            0
        };
        self.wall_ms = max_wall;
        self.counter = new_counter;
    }

    /// `{wall_ms:016x}-{counter:08x}-{node:08x}`
    pub fn format(&self) -> String {
        format!(
            "{:016x}-{:08x}-{:08x}",
            self.wall_ms, self.counter, self.node
        )
    }

    pub fn parse(s: &str) -> Result<Hlc> {
        let mut parts = s.split('-');
        let wall = parts.next().ok_or_else(|| bad(s))?;
        let counter = parts.next().ok_or_else(|| bad(s))?;
        let node = parts.next().ok_or_else(|| bad(s))?;
        if parts.next().is_some() {
            return Err(bad(s));
        }
        Ok(Hlc {
            wall_ms: u64::from_str_radix(wall, 16).map_err(|_| bad(s))?,
            counter: u32::from_str_radix(counter, 16).map_err(|_| bad(s))?,
            node: u32::from_str_radix(node, 16).map_err(|_| bad(s))?,
        })
    }

    /// Load the persisted device clock from `sync_state` (generating a `node_id`
    /// on first use).
    pub fn load(conn: &Connection) -> Result<Hlc> {
        let node = load_node_id(conn)?;
        let wall_ms = read_u64(conn, "hlc_wall_ms")?.unwrap_or(0);
        let counter = read_u32(conn, "hlc_counter")?.unwrap_or(0);
        Ok(Hlc {
            wall_ms,
            counter,
            node,
        })
    }

    /// Persist the clock's wall/counter back to `sync_state`.
    pub fn persist(&self, conn: &Connection) -> Result<()> {
        write_state(conn, "hlc_wall_ms", &self.wall_ms.to_string())?;
        write_state(conn, "hlc_counter", &self.counter.to_string())?;
        Ok(())
    }
}

/// Total order matching the lexicographic order of [`Hlc::format`].
impl Ord for Hlc {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.wall_ms, self.counter, self.node).cmp(&(other.wall_ms, other.counter, other.node))
    }
}
impl PartialOrd for Hlc {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn bad(s: &str) -> CrateError {
    CrateError::CloudSync(format!("invalid HLC: {s:?}"))
}

/// Read (or lazily generate) this device's node id, stored as 8 hex chars in
/// `sync_state`. Generated on the first mutation after migration — harmless even
/// when sync is never enabled.
pub fn load_node_id(conn: &Connection) -> Result<NodeId> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM sync_state WHERE key = 'node_id'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(s) = existing {
        return u32::from_str_radix(s.trim(), 16)
            .map_err(|e| CrateError::CloudSync(format!("invalid node_id {s:?}: {e}")));
    }
    // Low 32 bits of a v4 UUID; `| 1` guarantees a nonzero node id.
    let node = (uuid::Uuid::new_v4().as_u128() as u32) | 1;
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES ('node_id', ?1)",
        [format!("{node:08x}")],
    )?;
    Ok(node)
}

fn read_u64(conn: &Connection, key: &str) -> Result<Option<u64>> {
    let v: Option<String> = conn
        .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(v.and_then(|s| s.parse().ok()))
}

fn read_u32(conn: &Connection, key: &str) -> Result<Option<u32>> {
    let v: Option<String> = conn
        .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(v.and_then(|s| s.parse().ok()))
}

fn write_state(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_advances_wall_then_counter() {
        let mut h = Hlc::new(0, 0, 0xabcd);
        h.tick(1000);
        assert_eq!((h.wall_ms, h.counter), (1000, 0));
        // Same millisecond -> counter increments.
        h.tick(1000);
        assert_eq!((h.wall_ms, h.counter), (1000, 1));
        // Clock moved forward -> counter resets.
        h.tick(2000);
        assert_eq!((h.wall_ms, h.counter), (2000, 0));
    }

    #[test]
    fn format_is_lex_sortable() {
        let a = Hlc::new(1000, 0, 1).format();
        let b = Hlc::new(1000, 1, 1).format();
        let c = Hlc::new(1001, 0, 1).format();
        assert!(a < b);
        assert!(b < c);
        assert_eq!(Hlc::parse(&a).unwrap(), Hlc::new(1000, 0, 1));
    }

    #[test]
    fn empty_sentinel_sorts_below_real() {
        assert!("" < Hlc::new(0, 0, 0).format().as_str());
    }

    #[test]
    fn receive_is_monotonic() {
        let mut local = Hlc::new(1000, 5, 1);
        let remote = Hlc::new(1000, 9, 2);
        local.receive(&remote, 900);
        // Same wall on both, now_ms behind: counter = max(5,9)+1.
        assert_eq!((local.wall_ms, local.counter), (1000, 10));
    }
}
