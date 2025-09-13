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
                old_hash TEXT,
                last_modified INTEGER,
                status TEXT,
                existing BOOLEAN DEFAULT 0
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
        // Get current hash to set old_hash
        let mut stmt = self.conn.prepare("SELECT hash FROM files WHERE path = ?1")?;
        let old_hash: Option<String> = stmt.query_row(params![path], |row| row.get(0)).optional()?;

        // Insert if not exists
        self.conn.execute(
            "INSERT OR IGNORE INTO files (path, hash, old_hash, last_modified, status, existing) VALUES (?1, ?2, NULL, ?3, ?4, 1)",
            params![path, hash, last_modified, status],
        ).context("Failed to insert file if not exists")?;

        // Update existing record with old_hash set to previous hash
        self.conn.execute(
            "UPDATE files SET old_hash = ?1, hash = ?2, last_modified = ?3, status = ?4 WHERE path = ?5",
            params![old_hash, hash, last_modified, status, path],
        ).context("Failed to update file")?;

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

    /// Convenience helper: open DB at path and return tracked files.
    pub fn load_tracked_files(db_path: &Path) -> Result<Vec<TrackedFile>> {
        let db = ScuttleDb::new(db_path)?;
        db.get_tracked_files()
    }

    pub fn commit(&self, message: &str) -> Result<()> {
        let timestamp = Utc::now().timestamp();

        // Get current tracked files
        let current_files = self.get_tracked_files()?;

        // Map current files by path to old_hash
        let mut current_files_map = std::collections::HashMap::new();
        for file in &current_files {
            current_files_map.insert(file.path.clone(), file.hash.clone());
        }

        // Get last commit files and hashes from files table old_hash
        let mut stmt = self.conn.prepare("SELECT path, old_hash FROM files")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;

        let mut last_files_map = std::collections::HashMap::new();
        for row in rows {
            let (path, old_hash) = row?;
            last_files_map.insert(path, old_hash);
        }

        // Determine file changes
        // status can be "added", "modified", "deleted"
        let mut changes: Vec<(TrackedFile, &str)> = Vec::new();

        // Check for added or modified files by comparing current hash with old_hash
        for file in &current_files {
            // Skip files with status 'committed'
            if let Some(status) = &file.status {
                if status == "committed" {
                    continue;
                }
            }

            // If the file has been marked deleted, record it as deleted
            if let Some(status) = &file.status {
                if status == "deleted" {
                    changes.push((file.clone(), "deleted"));
                    continue;
                }
            }

            match last_files_map.get(&file.path) {
                Some(last_hash) => {
                    match last_hash {
                        Some(old_hash) => {
                            match &file.hash {
                                Some(current_hash) => {
                                    if current_hash != old_hash {
                                        changes.push((file.clone(), "modified"));
                                    }
                                }
                                None => {
                                    // No current hash, treat as unchanged
                                }
                            }
                        }
                        None => {
                            // No old_hash means new file
                            changes.push((file.clone(), "added"));
                        }
                    }
                }
                None => {
                    // New file
                    changes.push((file.clone(), "added"));
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
                self.conn.execute(
                    "UPDATE files SET status = 'committed' WHERE path = ?1",
                    params![file.path],
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
