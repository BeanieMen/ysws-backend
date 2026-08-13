ALTER TABLE projects
  ADD COLUMN banner_url TEXT,
  ADD COLUMN submission_status TEXT NOT NULL DEFAULT 'draft' CHECK (submission_status IN ('draft', 'submitted', 'under_review', 'approved', 'rejected')),
  ADD COLUMN submitted_at TIMESTAMPTZ;

CREATE INDEX projects_submission_status_idx ON projects(submission_status);
