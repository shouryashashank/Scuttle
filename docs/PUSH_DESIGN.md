Push design for Scuttle

Overview

This document describes a practical, safe, and incremental design for implementing `push` in Scuttle. The push operation uploads local repository state to a configured remote (initially Google Drive). The design prioritizes safety (no data loss), reliability (partial-failure recovery), and simplicity for the first iteration (assume local is ahead).

Goals

- Detect whether a remote repo exists and is managed by Scuttle.
- If remote does not exist: perform an initial full upload.
- If remote exists: compute a deterministic diff between the remote DB and local DB, and apply only the required changes (delete/upload/update) to remote storage.
- Safely replace the remote scuttle DB after applying file operations, minimizing the risk of leaving the remote in an inconsistent state.
- Keep the implementation iterative: implement a minimal safe subset first, then add robustness (locks, retries, folder IDs) in follow-ups.

Assumptions (first iteration)

- We support Google Drive only.
- The local repository is considered authoritative ("local is ahead"). No automatic merge from remote.
- Drive file and folder IDs are not yet persisted in the DB; we will look up remote files by path/name under a configured remote root folder.
- Users create remote repos by running `init` which stores a remote name and optionally a remote folder id later.

High-level algorithm

1. Resolve remote root folder
   - Read the remote configuration (remote name or folder id) from Scuttle config.
   - If a remote folder id exists, use it. Otherwise, search Drive for a folder matching the configured remote name and confirm.

2. Check for remote scuttle DB
   - Look for a file named `scuttle.db` inside the remote root folder (search by parent folder id + name), preferring `.scuttle/scuttle.db` if present.
   - If not found -> run initial upload: create project root folder (if needed), create `.scuttle` folder on remote, upload all tracked files and `.scuttle/scuttle.db`.

3. If remote scuttle DB found
   - Download remote DB to a temporary local path: `.scuttle/remote_scuttle.db.tmp`.
   - Open remote DB in read-only mode.

4. Compute diff between remote and local DBs
   - For each path in union(remote_files, local_files): classify as added / modified / deleted by comparing current hashes.
   - Use the `files` table `path` and `hash` columns. Ignore files marked `committed` vs `staged`—treat actual hashes as source of truth for file content differences.
   - Produce three lists: Added (present locally, not on remote), Modified (present both sides but hashes differ), Deleted (present on remote, not locally).

5. Map paths to remote file IDs (best-effort)
   - For each path that exists on remote, attempt to locate a Drive file id using path search under the remote root folder.
   - For absent remote files (added files) create parents as needed before upload (folder creation helper exists).

6. Apply remote operations (ordered for safety)
   - Deletes: delete remote files in Deleted list by file ID.
   - Uploads/Updates: for each Added/Modified file upload content into the corresponding remote folder. For modified, if file id is known, upload the new content (current approach uploads new and deletes old).
   - Best-effort: continue on recoverable failures, but record errors and abort if too many failures.

7. Swap remote DB safely
   - Capture existing remote `scuttle.db` id (if any) before uploading.
   - Upload local `.scuttle/scuttle.db` into remote `.scuttle`.
   - Verify upload success (response + size or checksum if available).
   - Delete the old `scuttle.db` only if it existed and its file id differs from the newly uploaded file.
   - Clean up any temporary remote objects.

8. Post-push updates
   - Optionally update local DB with remote file IDs for files that now have known ids (future improvement).
   - Print a push summary: counts of added/modified/deleted and any errors.

Safety and durability considerations

- Do not delete the remote scuttle DB until a fresh local DB has been uploaded and verified.
- Download the remote DB to a temp path to avoid clobbering the local DB.
- Upload local DB under a temporary name or capture old id first, then atomically replace remote DB when possible.
- Perform deletes before uploads to free names/paths, but ensure DB replacement happens only after uploads finish.
- Use retry/backoff on transient Drive API failures.

Incremental implementation plan (Milestones)

1. Scaffolding
   - Add a new public function `process_push(remote_name: Option<&str>) -> Result<()>` in `lib.rs`.
   - Add CLI command `push` that calls `process_push`.

2. Remote existence check + initial upload
   - Implement Drive helper to check for folder and `scuttle.db` presence.
   - Implement `initial_upload` path: create remote project folder and `.scuttle` folder on remote and upload all tracked files + DB.

3. Remote DB download and diff computation
   - Implement remote DB download to `.scuttle/remote_scuttle.db.tmp`.
   - Implement `diff_dbs(remote_db_path, local_db_path) -> (Vec<String> added, Vec<String> modified, Vec<String> deleted)` using `ScuttleDb` read-only instances.

4. Apply deltas: delete and upload
   - Implement Drive helpers: delete_by_id, upload_file_with_parent, find_file_id_by_path, ensure_remote_path.
   - Apply deletes then uploads.

5. Safe DB swap
   - Capture old ID, upload local DB, then delete old only if different from new.

6. Finalize and improve
   - Persist remote file IDs in DB (add column `remote_id`) and incrementally maintain it.
   - Add push.lock to prevent concurrent pushes.
   - Add better error reporting and retry logic.

Schema and API changes recommended

- files table: add `remote_id TEXT` column to store Drive file IDs for faster subsequent lookups.
- `src/google_drive_api_client.rs`: expose helpers to find/upload/delete files by parent id and path; helpers added for folder creation and ensure-remote-path.

Example minimal push behavior for MVP

- If remote has no scuttle DB -> upload everything (initial upload) preserving folder structure.
- If remote has scuttle DB -> download remote DB, compute path-level diffs by hash, delete remote-only paths, upload new/modified paths into matching folder structure, upload DB and replace remote DB safely.

Progress implemented so far (branch: feature/push)

The repo already contains a functional, iterative implementation of the push flow. Key implemented pieces:

- CLI and scaffold
  - `push` subcommand added to `src/main.rs`.
  - `process_push(remote_name: Option<&str>)` implemented in `src/lib.rs`.

- Drive helper primitives (Google Drive)
  - `get_drive_client` — creates and authenticates a Drive client.
  - `find_folder_by_name` — searches for a folder by name.
  - `find_file_in_folder` — searches for a file by name under a given parent id.
  - `upload_file_with_parent` — uploads a file and places it under a given parent folder id (supports shared drives).
  - `download_file_by_id` — downloads a file by its file ID.
  - `delete_file_by_id` — deletes a file by id.
  - `create_folder` — creates a folder resource (implemented using an empty upload as the generated client requires it).
  - `ensure_remote_path` — ensures nested folders exist under a root id, creating them as needed.

- DB helpers and diff
  - `ScuttleDb::load_tracked_files(db_path)` added in `src/sqlite_db.rs`.
  - `ScuttleDb::diff_dbs(remote_db_path, local_db_path)` implemented to compute added/modified/deleted paths by comparing `files` table hashes.

- Initial upload implementation (preserves folder structure)
  - When remote root isn't found, `process_push` now creates the remote root folder and uses `ensure_remote_path` to replicate local folder structure on the remote.
  - All tracked files are uploaded into matching remote folders using `upload_file_with_parent`.
  - The local `.scuttle/scuttle.db` is uploaded into remote `.scuttle`.

- Apply-deltas implementation
  - When a remote DB exists, `process_push` downloads it, computes diffs, then:
    - Deletes remote-only files by locating remote IDs via path traversal and `find_file_in_folder` + `delete_file_by_id`.
    - Uploads added/modified files into matching remote folders (creating folders as needed).
    - Performs a safer DB swap: captures the old `scuttle.db` id before upload, uploads the new DB, and deletes the old id only if it differs from the newly uploaded file id.

Notes, caveats and remaining TODOs

- The implementation is intentionally conservative: it assumes "local is ahead" for now and doesn't attempt merges. Conflicts must be detected and handled in later iterations.

- The generated Google Drive client has some quirks (private `doit`/`execute` differences). The code uses the upload-based folder creation approach which works with the generated client and `supports_all_drives` where appropriate.

- Current push is best-effort and can leave partial state on failure. Improvements to make next:
  - Verify uploaded DB integrity (size/checksum) before deleting the remote DB.
  - Add retries/backoff and transactional rollback where possible.
  - Persist remote file IDs (`remote_id` column) to speed up future pushes and avoid repeated lookups.
  - Implement push.lock (remote) to prevent concurrent pushes.

Next recommended commits

1. Add checksum verification for DB uploads and use it to decide whether to delete the previous DB.
2. Persist `remote_id` column on `files` table and update it after uploads.
3. Implement a safe rollback strategy when applying deltas fails mid-way.
4. Add unit tests for `diff_dbs` and integration tests for the push flow using a small sample repo.

If you want, I can now:
- Remove any duplicate or unused helper functions and tidy imports, or
- Implement checksum verification for DB swap next, or
- Add persistent `remote_id` support to the DB schema and update code to write file ids after upload.

Tell me which task you want me to do next and I will implement it.
