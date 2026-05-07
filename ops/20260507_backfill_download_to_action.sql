-- Backfill historical download_events into action_events.
--
-- Why this exists:
--   The "call to action track" commit renamed download_events -> action_events
--   and added a `domain` column. Old rows have no domain, so you must pick one
--   per row before they will show up in the dashboard (which filters by domain).
--
-- Run this ONCE, against a backed-up database, while the server is stopped or
-- read-only. Verify counts before and after.
--
-- Usage:
--   1. Back up first:
--        sqlite3 ram.db ".backup 'ram.db.bak'"
--   2. Edit the WHERE / CASE clause below to match your deployment (see notes).
--   3. Apply:
--        sqlite3 ram.db < migrations/20260507_backfill_download_to_action.sql
--   4. Verify:
--        sqlite3 ram.db "SELECT domain, COUNT(*) FROM action_events GROUP BY domain;"
--   5. Once you've confirmed the dashboard looks right, you may drop the old
--      table to reclaim space:
--        DROP TABLE download_events;
--      (Do this in a separate, deliberate step — not as part of this script.)

BEGIN;

-- Sanity check: action_events must already exist (created on first server boot
-- after the rename). If this errors, start the new server once, then re-run.
SELECT COUNT(*) FROM action_events;

-- Choose ONE of the strategies below by uncommenting it.
--
-- ----------------------------------------------------------------------------
-- Strategy A: single-site deployment. All historical downloads belong to one
-- domain. Replace 'example.com' with yours.
-- ----------------------------------------------------------------------------
-- INSERT INTO action_events (domain, name, label, referrer, created_at)
-- SELECT
--     'example.com'                                AS domain,
--     app_name                                     AS name,
--     TRIM(COALESCE(platform, '') || ' ' || COALESCE(version, '')) AS label,
--     referrer,
--     created_at
-- FROM download_events;

-- ----------------------------------------------------------------------------
-- Strategy B: multi-site. Map domain from the Referer column when present,
-- fall back to a default. Adjust the CASE arms to your known domains.
-- ----------------------------------------------------------------------------
-- INSERT INTO action_events (domain, name, label, referrer, created_at)
-- SELECT
--     CASE
--         WHEN referrer LIKE 'https://app1.example.com/%' THEN 'app1.example.com'
--         WHEN referrer LIKE 'https://app2.example.com/%' THEN 'app2.example.com'
--         ELSE 'unknown'
--     END                                          AS domain,
--     app_name                                     AS name,
--     TRIM(COALESCE(platform, '') || ' ' || COALESCE(version, '')) AS label,
--     referrer,
--     created_at
-- FROM download_events;

-- ----------------------------------------------------------------------------
-- Verification: counts should match (old rows -> new rows inserted).
-- ----------------------------------------------------------------------------
-- SELECT 'old', COUNT(*) FROM download_events
-- UNION ALL
-- SELECT 'new (this backfill, by created_at range)', COUNT(*)
--   FROM action_events
--   WHERE created_at <= (SELECT MAX(created_at) FROM download_events);

COMMIT;
