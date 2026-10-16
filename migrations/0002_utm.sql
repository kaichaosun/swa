-- Campaign tags from the landing page's URL (?utm_source=…), empty when absent.

ALTER TABLE page_views ADD COLUMN utm_source TEXT NOT NULL DEFAULT '';
ALTER TABLE page_views ADD COLUMN utm_medium TEXT NOT NULL DEFAULT '';
ALTER TABLE page_views ADD COLUMN utm_campaign TEXT NOT NULL DEFAULT '';
