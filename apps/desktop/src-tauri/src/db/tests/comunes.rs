//! Base de datos temporal y limpieza, compartidas por todos los bloques.

use crate::db::Database;
use uuid::Uuid;

pub(super) fn test_database() -> Database {
    let path = std::env::temp_dir().join(format!(
        "chatygpt-db-test-{}.sqlite",
        Uuid::new_v4().simple()
    ));
    Database::open(path).expect("test database should open")
}

pub(super) fn cleanup(database: &Database) {
    let path = database.path().to_path_buf();
    for candidate in [
        path.clone(),
        path.with_extension("sqlite-wal"),
        path.with_extension("sqlite-shm"),
    ] {
        let _ = std::fs::remove_file(candidate);
    }
}
