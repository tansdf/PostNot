use sqlx::SqlitePool;

pub struct AppState {
    db: SqlitePool,
}

impl AppState {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &SqlitePool {
        &self.db
    }
}
