use anyhow::{Result, Context};
use rusqlite::{Connection, params};
use rusqlite::OptionalExtension;
use std::path::Path;
use chrono::Utc;

pub struct ScuttleDb {
    conn: Connection,
}

#[derive(Debug, Clone)]
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
                timestamp INTEGER,
                added_files TEXT,
                updated_files TEXT,
                deleted_files TEXT
            );
            CREATE TABLE IF NOT EXISTS commit_files (
                commit_id INTEGER,
                file_id INTEGER,
                status TEXT,
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

    fn get_last_commit_files(&self) -> Result<Vec<(String, Option<String>)>> {
        // Get the files and their hashes from the last commit
        let mut stmt = self.conn.prepare(
            "SELECT f.path, f.hash FROM files f
             JOIN commit_files cf ON f.id = cf.file_id
             JOIN commits c ON cf.commit_id = c.id
             WHERE c.id = (SELECT MAX(id) FROM commits)"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let mut last_files = Vec::new();
        for row in rows {
            last_files.push(row?);
        }
        Ok(last_files)
    }

    pub fn commit(&self, message: &str) -> Result<()> {
        let timestamp = Utc::now().timestamp();

        // Get current tracked files
        let current_files = self.get_tracked_files()?;

        // Get last commit files and hashes
        let last_files = self.get_last_commit_files().unwrap_or_default();

        // Map last files for quick lookup
        let mut last_files_map = std::collections::HashMap::new();
        for (path, hash) in last_files {
            last_files_map.insert(path, hash);
        }

        // Determine file changes
        // status can be "added", "modified", "deleted"
        let mut changes: Vec<(TrackedFile, &str)> = Vec::new();

        // Check for added or modified files
        for file in &current_files {
            match last_files_map.get(&file.path) {
                None => changes.push((file.clone(), "added")),
                Some(last_hash) => {
                    if last_hash.as_ref() != file.hash.as_ref() {
                        changes.push((file.clone(), "modified"));
                    }
                }
            }
        }

        // Check for deleted files
        for (path, _) in &last_files_map {
            if !current_files.iter().any(|f| &f.path == path) {
                // Create a dummy TrackedFile for deleted
                let deleted_file = TrackedFile {
                    path: path.clone(),
                    hash: None,
                    last_modified: None,
                    status: None,
                };
                changes.push((deleted_file, "deleted"));
            }
        }

        // Separate file paths by change type
        let mut added_paths = Vec::new();
        let mut updated_paths = Vec::new();
        let mut deleted_paths = Vec::new();

        // Insert commit record with placeholders for file lists
        self.conn.execute(
            "INSERT INTO commits (message, timestamp, added_files, updated_files, deleted_files) VALUES (?1, ?2, '', '', '')",
            params![message, timestamp],
        )?;

        let commit_id = self.conn.last_insert_rowid();

        // Insert commit_files entries and update files table
        for (file, status) in &changes {
            // For added or modified, update files table and set status to 'committed'
            if *status != "deleted" {
                self.add_file(
                    &file.path,
                    file.hash.as_deref().unwrap_or("") ,
                    file.last_modified.unwrap_or(0),
                    "committed"
                )?;
            } else {
                // For deleted, remove from files table
                self.conn.execute(
                    "DELETE FROM files WHERE path = ?1",
                    params![file.path],
                )?;
            }

            // Get file id
            let mut stmt = self.conn.prepare("SELECT id FROM files WHERE path = ?1")?;
            let file_id: Option<i64> = stmt.query_row(params![file.path], |row| row.get(0)).optional()?;

            // Insert into commit_files
            self.conn.execute(
                "INSERT INTO commit_files (commit_id, file_id, status) VALUES (?1, ?2, ?3)",
                params![commit_id, file_id, status],
            )?;

            // Collect file paths by status
            match *status {
                "added" => added_paths.push(file.path.clone()),
                "modified" => updated_paths.push(file.path.clone()),
                "deleted" => deleted_paths.push(file.path.clone()),
                _ => {}
            }
        }

        // Update commit record with file path lists
        let added_str = added_paths.join(",");
        let updated_str = updated_paths.join(",");
        let deleted_str = deleted_paths.join(",");

        self.conn.execute(
            "UPDATE commits SET added_files = ?1, updated_files = ?2, deleted_files = ?3 WHERE id = ?4",
            params![added_str, updated_str, deleted_str, commit_id],
        )?;

        Ok(())
    }

    // Additional methods for querying can be added here
}
