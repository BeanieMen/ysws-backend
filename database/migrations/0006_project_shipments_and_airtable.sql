ALTER TABLE users ADD COLUMN airtable_participant_record_id TEXT UNIQUE;

CREATE TABLE project_shipments (
    project_id UUID PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    shipped_at TIMESTAMPTZ,
    project_approval_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (project_approval_status IN ('pending', 'approved', 'rejected', 'changes_requested')),
    project_reviewed_at TIMESTAMPTZ,
    project_reviewer_id UUID REFERENCES users(id) ON DELETE SET NULL,
    fraud_approval_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (fraud_approval_status IN ('pending', 'approved', 'rejected')),
    fraud_reviewed_at TIMESTAMPTZ,
    airtable_project_record_id TEXT UNIQUE,
    airtable_synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Every project has one lifecycle record, so admins can approve an unshipped
-- project while only shipped rows are sent to Airtable.
INSERT INTO project_shipments (project_id, shipped_at)
SELECT id, shipped_at FROM projects;

CREATE INDEX project_shipments_fraud_approval_status_idx
    ON project_shipments(fraud_approval_status);
