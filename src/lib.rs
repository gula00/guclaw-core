use napi::bindgen_prelude::*;
use napi_derive::napi;
use rusqlite::{Connection, OptionalExtension};
use std::sync::{Mutex, MutexGuard};

mod mappers;
mod ops;
mod types;
pub use types::*;

#[napi]
pub struct DbHandle {
    path: String,
    state: Mutex<DbState>,
}

struct DbState {
    conn: Option<Connection>,
    closed: bool,
}

#[napi]
impl DbHandle {
    fn lock_state(&self) -> Result<MutexGuard<'_, DbState>> {
        self.state
            .lock()
            .map_err(|_| Error::from_reason("db handle mutex is poisoned".to_string()))
    }

    fn with_connection<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> rusqlite::Result<T>,
    {
        let mut state = self.lock_state()?;
        if state.closed {
            return Err(Error::from_reason(
                "db handle is already closed".to_string(),
            ));
        }

        let conn = state
            .conn
            .as_mut()
            .ok_or_else(|| Error::from_reason("db connection is not available".to_string()))?;

        f(conn).map_err(|err| Error::from_reason(format!("sqlite operation failed: {err}")))
    }

    #[napi]
    pub fn db_path(&self) -> String {
        self.path.clone()
    }

    #[napi]
    pub fn ping(&self) -> Result<PingResult> {
        let sqlite_version: Option<String> = self.with_connection(|conn| {
            conn.query_row("SELECT sqlite_version()", [], |row| row.get(0))
                .optional()
        })?;

        Ok(PingResult {
            ok: true,
            sqlite_version: sqlite_version.unwrap_or_else(|| "unknown".to_string()),
        })
    }

    #[napi]
    pub fn close(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.conn = None;
            state.closed = true;
        }
    }
}

#[napi]
pub fn open_db(path: String) -> Result<DbHandle> {
    let conn = Connection::open(&path)
        .map_err(|err| Error::from_reason(format!("failed to open sqlite database: {err}")))?;

    conn.execute_batch("PRAGMA foreign_keys = ON")
        .map_err(|err| Error::from_reason(format!("sqlite operation failed: {err}")))?;

    Ok(DbHandle {
        path,
        state: Mutex::new(DbState {
            conn: Some(conn),
            closed: false,
        }),
    })
}

#[cfg(test)]
include!("tests.rs");
