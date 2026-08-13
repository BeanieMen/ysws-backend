CREATE TABLE users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE hackatime_connections (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL,
    access_token_ciphertext TEXT NOT NULL,
    connected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE projects (
    id UUID PRIMARY KEY,
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 120),
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX projects_owner_id_created_at_idx ON projects(owner_id, created_at DESC);

CREATE TABLE project_hackatime_projects (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    hackatime_project_name TEXT NOT NULL CHECK (char_length(hackatime_project_name) BETWEEN 1 AND 255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, hackatime_project_name)
);
CREATE INDEX project_hackatime_project_name_idx ON project_hackatime_projects(hackatime_project_name);

CREATE TABLE attendance_registrations (
    id UUID PRIMARY KEY,
    event_id UUID NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    attendee_email TEXT NOT NULL,
    attendee_first_name TEXT NOT NULL,
    attendee_last_name TEXT NOT NULL,
    attend_participant_id TEXT,
    provider_response JSONB,
    status TEXT NOT NULL CHECK (status IN ('pending', 'registered', 'failed')),
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (event_id, user_id)
);
