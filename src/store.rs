use crate::error::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path, page_cache_kib: i64) -> Result<Self, ToolError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::Store(e.to_string()))?;
        }
        let conn = Connection::open(path).map_err(|e| ToolError::Store(e.to_string()))?;
        // A negative cache_size tells SQLite that the value is in kibibytes.
        conn.pragma_update(None, "cache_size", -page_cache_kib.min(2048))
            .map_err(|e| ToolError::Store(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cache (
                cache_key TEXT PRIMARY KEY,
                body TEXT NOT NULL,
                expires_at INTEGER NOT NULL,
                last_used INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS quota (
                name TEXT PRIMARY KEY,
                spent INTEGER NOT NULL,
                day TEXT NOT NULL
            );",
        )
        .map_err(|e| ToolError::Store(e.to_string()))?;
        Ok(Self { conn })
    }

    pub fn get_fresh(&self, key: &str, now: i64) -> Result<Option<String>, ToolError> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT body, expires_at FROM cache WHERE cache_key = ?1",
                params![key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| ToolError::Store(e.to_string()))?;
        let Some((body, expires_at)) = row else {
            return Ok(None);
        };
        if expires_at <= now {
            self.conn
                .execute("DELETE FROM cache WHERE cache_key = ?1", params![key])
                .map_err(|e| ToolError::Store(e.to_string()))?;
            return Ok(None);
        }
        self.conn
            .execute(
                "UPDATE cache SET last_used = ?1 WHERE cache_key = ?2",
                params![now, key],
            )
            .map_err(|e| ToolError::Store(e.to_string()))?;
        Ok(Some(body))
    }

    pub fn put(
        &self,
        key: &str,
        body: &str,
        expires_at: i64,
        now: i64,
        max_entries: i64,
    ) -> Result<(), ToolError> {
        self.conn
            .execute(
                "INSERT INTO cache (cache_key, body, expires_at, last_used)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(cache_key) DO UPDATE SET
                    body = excluded.body,
                    expires_at = excluded.expires_at,
                    last_used = excluded.last_used",
                params![key, body, expires_at, now],
            )
            .map_err(|e| ToolError::Store(e.to_string()))?;
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM cache", [], |row| row.get(0))
            .map_err(|e| ToolError::Store(e.to_string()))?;
        if count > max_entries {
            self.conn
                .execute(
                    "DELETE FROM cache WHERE cache_key IN (
                        SELECT cache_key FROM cache ORDER BY last_used ASC LIMIT ?1
                    )",
                    params![count - max_entries],
                )
                .map_err(|e| ToolError::Store(e.to_string()))?;
        }
        Ok(())
    }

    pub fn can_spend(
        &self,
        counter: &str,
        cost: i64,
        budget: i64,
        today: &str,
    ) -> Result<bool, ToolError> {
        Ok(self.spent_today(counter, today)? + cost <= budget)
    }

    pub fn remaining(&self, counter: &str, budget: i64, today: &str) -> Result<i64, ToolError> {
        Ok(budget - self.spent_today(counter, today)?)
    }

    pub fn debit(
        &self,
        counter: &str,
        cost: i64,
        budget: i64,
        today: &str,
    ) -> Result<(), ToolError> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| ToolError::Store(e.to_string()))?;
        let spent = spent_today_tx(&tx, counter, today)?;
        if spent + cost > budget {
            return Err(ToolError::QuotaExhausted {
                counter: if counter == "search" {
                    "search"
                } else {
                    "non_search"
                },
                remaining: budget - spent,
                cost,
            });
        }
        tx.execute(
            "INSERT INTO quota (name, spent, day) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET spent = ?2, day = ?3",
            params![counter, spent + cost, today],
        )
        .map_err(|e| ToolError::Store(e.to_string()))?;
        tx.commit().map_err(|e| ToolError::Store(e.to_string()))?;
        Ok(())
    }

    fn spent_today(&self, counter: &str, today: &str) -> Result<i64, ToolError> {
        spent_today_tx(&self.conn, counter, today)
    }
}

fn spent_today_tx(conn: &Connection, counter: &str, today: &str) -> Result<i64, ToolError> {
    let row: Option<(i64, String)> = conn
        .query_row(
            "SELECT spent, day FROM quota WHERE name = ?1",
            params![counter],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| ToolError::Store(e.to_string()))?;
    match row {
        Some((spent, day)) if day == today => Ok(spent),
        _ => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> Store {
        let path = std::env::temp_dir().join(format!(
            "yt-store-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        Store::open(&path, 2048).unwrap()
    }

    #[test]
    fn open_caps_page_cache_at_two_mibibytes() {
        let path = std::env::temp_dir().join(format!(
            "yt-store-cache-cap-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let store = Store::open(&path, 4096).unwrap();

        let cache_size: i64 = store
            .conn
            .pragma_query_value(None, "cache_size", |row| row.get(0))
            .unwrap();
        assert_eq!(cache_size, -2048);
    }

    #[test]
    fn fresh_row_hits_and_expired_row_misses() {
        let store = temp_store();
        store.put("k", "{\"items\":[]}", 200, 100, 10).unwrap();
        assert!(store.get_fresh("k", 150).unwrap().is_some());
        assert!(store.get_fresh("k", 200).unwrap().is_none());
    }

    #[test]
    fn debit_stops_at_the_budget_and_resets_on_a_new_day() {
        let store = temp_store();
        store.debit("search", 1, 2, "2026-11-01").unwrap();
        store.debit("search", 1, 2, "2026-11-01").unwrap();
        assert!(store.debit("search", 1, 2, "2026-11-01").is_err());
        store.debit("search", 1, 2, "2026-11-02").unwrap();
    }

    #[test]
    fn search_debit_does_not_change_non_search() {
        let store = temp_store();
        store.debit("search", 1, 100, "2026-11-01").unwrap();
        assert!(store
            .can_spend("non_search", 1, 10_000, "2026-11-01")
            .unwrap());
    }

    #[test]
    fn remaining_resets_before_the_new_day_is_written() {
        let store = temp_store();
        store.debit("search", 3, 10, "2026-11-01").unwrap();
        assert_eq!(store.remaining("search", 10, "2026-11-01").unwrap(), 7);
        assert_eq!(store.remaining("search", 10, "2026-11-02").unwrap(), 10);
    }
}
