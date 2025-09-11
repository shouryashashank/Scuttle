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
   - Look for a file named `scuttle.db` inside the remote root folder (search by parent folder id + name).
   - If not found -> run initial upload: create `.scuttle` folder on remote, upload all tracked files and `.scuttle/scuttle.db`.

3. If remote scuttle DB found
   - Download remote DB to a temporary local path: `.scuttle/remote_scuttle.db.tmp`.
   - Open remote DB in read-only mode.

4. Compute diff between remote and local DBs
   - For each path in union(remote_files, local_files): classify as added / modified / deleted by comparing current hashes.
   - Use the `files` table `path` and `hash` columns. Ignore files marked `committed` vs `staged`—treat actual hashes as source of truth for file content differences.
   - Produce three lists: Added (present locally, not on remote), Modified (present both sides but hashes differ), Deleted (present on remote, not locally).

5. Map paths to remote file IDs (best-effort)
   - For each path that exists on remote, attempt to locate a Drive file id using path search under the remote root folder.
   - For absent remote files (added files) create parents as needed before upload.

6. Apply remote operations (ordered for safety)
   - Deletes: delete remote files in Deleted list by file ID.
   - Uploads/Updates: for each Added/Modified file upload content. For modified, if file id is known, replace the remote file (drive update) or upload new and delete old.
   - Best-effort: continue on recoverable failures, but record errors and abort if too many failures.

7. Swap remote DB safely
   - Upload local `.scuttle/scuttle.db` as `scuttle.db.tmp` to remote.
   - Verify upload success (response + size or checksum if available).
   - Delete remote `scuttle.db` and rename/move `scuttle.db.tmp` to `scuttle.db` (or delete old and write new with same name if API does not support rename).
   - Clean up any temporary remote objects.

8. Post-push updates
   - Optionally update local DB with remote file IDs for files that now have known ids (future improvement).
   - Print a push summary: counts of added/modified/deleted and any errors.

Safety and durability considerations

- Do not delete the remote scuttle DB until a fresh local DB has been uploaded and verified.
- Download the remote DB to a temp path to avoid clobbering the local DB.
- Upload local DB under a temporary name, then atomically replace remote DB when possible.
- Perform deletes before uploads to free names/paths, but ensure DB replacement happens only after uploads finish.
- Use retry/backoff on transient Drive API failures.

Incremental implementation plan (Milestones)

1. Scaffolding
   - Add a new public function `process_push(remote_name: Option<&str>) -> Result<()>` in `lib.rs`.
   - Add CLI command `push` that calls `process_push`.

2. Remote existence check + initial upload
   - Implement Drive helper to check for folder and `scuttle.db` presence.
   - Implement `initial_upload` path: create `.scuttle` folder on remote and upload all tracked files + DB.

3. Remote DB download and diff computation
   - Implement remote DB download to `.scuttle/remote_scuttle.db.tmp`.
   - Implement `diff_dbs(remote_db_path, local_db_path) -> (Vec<String> added, Vec<String> modified, Vec<String> deleted)` using `ScuttleDb` read-only instances.

4. Apply deltas: delete and upload
   - Implement Drive helpers: delete_by_id, upload_file_with_parent, find_file_id_by_path.
   - Apply deletes then uploads.

5. Safe DB swap
   - Upload `.scuttle/scuttle.db` as `scuttle.db.tmp`, verify, then replace.

6. Finalize and improve
   - Persist remote file IDs in DB (add column `remote_id`) and incrementally maintain it.
   - Add push.lock to prevent concurrent pushes.
   - Add better error reporting and retry logic.

Schema and API changes recommended

- files table: add `remote_id TEXT` column to store Drive file IDs for faster subsequent lookups.
- google_drive_api_client.rs: expose helpers to find/upload/update/delete files by parent id and path.

Example minimal push behavior for MVP

- If remote has no scuttle DB -> upload everything (initial upload).
- If remote has scuttle DB -> download remote DB, compute path-level diffs by hash, delete remote-only paths, upload new/modified paths, upload DB temporary and replace remote DB.

Progress implemented so far

The following items from the design have already been implemented in the codebase on branch `feature/push`:

- CLI and scaffold
  - Added a new `push` subcommand in `src/main.rs`.
  - Added a scaffold `process_push(remote_name: Option<&str>)` in `src/lib.rs` which validates configuration and orchestrates the push flow.

- Drive helper primitives (Google Drive)
  - Implemented helper functions in `src/google_drive_api_client.rs`:
    - `get_drive_client` — creates and authenticates a Drive client.
    - `find_folder_by_name` — searches for a folder by name.
    - `find_file_in_folder` — searches for a file by name under a given parent id.
    - `upload_file_with_parent` — uploads a file and places it under a given parent folder id (honors supports_all_drives when parent provided).
    - `download_file_by_id` — downloads a file by its file ID.
    - `delete_file_by_id` — (added, will be used later for deletes).
    - `create_folder` — initial work done (requires a small call-site API fix; see TODOs).

- DB helpers and separation
  - Added `ScuttleDb::load_tracked_files(db_path: &Path) -> Result<Vec<TrackedFile>>` in `src/sqlite_db.rs` to centralize reading tracked files from the DB.

- Initial upload implementation
  - `process_push` now performs an initial upload flow when the remote root folder is not found:
    - Creates a remote root folder (attempts to use `create_folder`).
    - Loads tracked files from the local DB and uploads each existing file into the created remote folder using `upload_file_with_parent`.
    - Uploads local `.scuttle/scuttle.db` into the same remote folder.
    - Prints progress and a summary (uploaded/skipped counts).

What still remains / TODOs

- Drive API call adjustments
  - The `create_folder` implementation needs a small fix to call the generated client correctly for folder creation (the generated client API method signatures must be invoked exactly). This is a trivial code fix and will complete true folder creation support.

- Diff & apply logic
  - Implement the DB diff function to compute added/modified/deleted lists between remote DB and local DB.
  - Implement mapping from DB paths to remote file IDs (best-effort lookup or persist `remote_id` column in DB).
  - Implement applying deltas: deletes, uploads (modify vs replace), with retries and error handling.

- Safe DB swap
  - Upload local DB under a temporary name and then atomically replace remote `scuttle.db`.

- Concurrency and robustness
  - Add remote push lock, retries/backoff, and conflict detection (future iterations).

Next recommended steps (next commits)

1. Fix the `create_folder` Drive call so `process_push` reliably creates the remote root folder.
2. Implement `diff_dbs(remote_db_path, local_db_path)` using read-only `ScuttleDb` instances and return (added, modified, deleted) lists.
3. Implement `apply_deltas` in `lib.rs` that uses the Drive helpers to delete and upload files and then perform the safe DB swap.
4. Add tests for `diff_dbs` using two small DB files to verify classification.

If you want, I can fix the `create_folder` call now and then implement the DB diff function next.

Updates based on user feedback

- Multi-service support: Google Drive is the initial target for the MVP only. The design is written to be service-agnostic where possible; Drive-specific helpers and behavior should be isolated behind an API layer so Dropbox, OneDrive, SMB, or other services can be added later.

- Conflict model: For the first iteration we keep the simple "local is ahead" assumption to make push deterministic and safe. However, the design acknowledges that conflicts are possible in multi-client scenarios even with a single branch. Future milestones should add:
  - Conflict detection (when remote DB and local DB both have diverging changes).
  - A basic conflict resolution strategy (e.g., abort push and surface conflicts to user, or simple automatic merge rules configurable by user).
  - Locking/optimistic concurrency checks to reduce the likelihood of conflicting pushes.

- Extensibility: Keep Drive-specific logic in `google_drive_api_client.rs` and depend on a small set of helpers (find folder/file by path, download/upload by id, delete by id). Higher-level push orchestration in `lib.rs` should call these helpers through a small trait or wrapper in a future refactor to support more services.

If this looks good I will:
- Add a short TODO list to the document describing the immediate code changes required for milestone 1 (CLI + scaffold) and milestone 2 (DB diff), and then implement the chosen milestone.
