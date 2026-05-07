# ops/

Manual, one-shot SQL scripts. **These are NOT run by the server.** They exist
to document operational changes (backfills, data fixes, drops) so a human can
run them deliberately and then leave them in the repo as a record.

If you want a script to run automatically on every boot, it belongs in
`migrations/`, not here.

## Conventions

- File name: `YYYYMMDD_short_description.sql`.
- Each file should start with a comment block explaining: why it exists, when
  to run it, and how to verify the result.
- Wrap mutations in `BEGIN; ... COMMIT;` so a partial run rolls back.
- Always back up the database before running:
  ```sh
  sqlite3 ram.db ".backup 'ram.db.bak-$(date +%F)'"
  ```
- Apply with:
  ```sh
  sqlite3 ram.db < ops/<filename>.sql
  ```

Once a script has been applied to all environments and there is no future use,
it can be deleted in a follow-up commit (git history is the record).
