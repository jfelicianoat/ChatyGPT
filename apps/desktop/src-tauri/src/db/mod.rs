use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::error::AppError;

const INITIAL_MIGRATION: &str = include_str!("../../migrations/0001_initial.sql");
const RECOVER_NON_TERMINAL_TASKS: &str =
    include_str!("../../queries/recover_non_terminal_tasks.sql");
pub const SCHEMA_VERSION: i64 = 1;

pub struct Database {
    path: PathBuf,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref().to_path_buf();
        let mut connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Self::migrate(&mut connection)?;
        Ok(Self { path })
    }

    fn migrate(connection: &mut Connection) -> Result<(), AppError> {
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < SCHEMA_VERSION {
            let transaction = connection.transaction()?;
            transaction.execute_batch(INITIAL_MIGRATION)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        Ok(())
    }

    fn connect(&self) -> Result<Connection, AppError> {
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        Ok(connection)
    }

    pub fn schema_version(&self) -> Result<i64, AppError> {
        Ok(self.connect()?.pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    pub fn recover_non_terminal_tasks(&self) -> Result<usize, AppError> {
        let connection = self.connect()?;
        let changed = connection.execute(RECOVER_NON_TERMINAL_TASKS, [])?;
        Ok(changed)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
