-- Shipping is a separate, auditable project lifecycle event.  A timestamp is
-- preferable to inferring visibility from a mutable review status.
ALTER TABLE projects ADD COLUMN shipped_at TIMESTAMPTZ;

-- Preserve access to projects submitted before this migration was introduced.
UPDATE projects
SET shipped_at = COALESCE(submitted_at, updated_at)
WHERE submission_status <> 'draft' OR submitted_at IS NOT NULL;

CREATE INDEX projects_shipped_at_idx ON projects(shipped_at DESC) WHERE shipped_at IS NOT NULL;
