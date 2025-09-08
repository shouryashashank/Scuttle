use anyhow::{Result, Context};
use rusqlite::{Connection, params};
use std::path::Path;

pub struct ScuttleDb {
    conn: Connection,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct TrackedFile {
    pub path: String,
    pub hash: Option<String>,
    pub last_modified: Option<i64>,
    pub status: Option<String>,
}

impl ScuttleDb {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path).context("Failed to open SQLite database")?;
        let db = ScuttleDb { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                hash TEXT,
                last_modified INTEGER,
                status TEXT
            );
            CREATE TABLE IF NOT EXISTS commits (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message TEXT,
                timestamp INTEGER
            );
            CREATE TABLE IF NOT EXISTS commit_files (
                commit_id INTEGER,
                file_id INTEGER,
                FOREIGN KEY(commit_id) REFERENCES commits(id),
                FOREIGN KEY(file_id) REFERENCES files(id)
            );"
        ).context("Failed to create tables")?;
        Ok(())
    }

    pub fn add_file(&self, path: &str, hash: &str, last_modified: i64, status: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO files (path, hash, last_modified, status) VALUES (?1, ?2, ?3, ?4)",
            params![path, hash, last_modified, status],
        ).context("Failed to add or update file")?;
        Ok(())
    }

    pub fn get_tracked_files(&self) -> Result<Vec<TrackedFile>> {
        let mut stmt = self.conn.prepare("SELECT path, hash, last_modified, status FROM files")?;
        let file_iter = stmt.query_map([], |row| {
            Ok(TrackedFile {
                path: row.get(0)?,
                hash: row.get(1)?,
                last_modified: row.get(2)?,
                status: row.get(3)?,
            })
        })?;

        let mut files = Vec::new();
        for file in file_iter {
            files.push(file?);
        }
        Ok(files)
    }

    // Additional methods for commits and querying can be added here
}
