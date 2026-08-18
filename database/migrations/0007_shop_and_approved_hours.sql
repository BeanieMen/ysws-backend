-- Approved time is stored in whole minutes so balance and purchase arithmetic
-- is exact. The public API presents this as hours for people.
CREATE TABLE user_wallets (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    available_minutes BIGINT NOT NULL DEFAULT 0 CHECK (available_minutes >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE project_credit_awards (
    project_id UUID PRIMARY KEY REFERENCES projects(id) ON DELETE RESTRICT,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    credited_minutes BIGINT NOT NULL CHECK (credited_minutes >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX project_credit_awards_user_id_idx ON project_credit_awards(user_id, created_at DESC);

CREATE TABLE shop_items (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 120),
    description TEXT NOT NULL CHECK (char_length(description) BETWEEN 1 AND 1000),
    price_minutes BIGINT NOT NULL CHECK (price_minutes > 0),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- This stable id makes the initial ticket safe to reference from environment-
-- independent frontend code while still keeping all purchase authority in DB.
INSERT INTO shop_items (id, slug, name, description, price_minutes)
VALUES (
    'a4bba639-934d-48f4-9e51-d6328b0a7d54',
    'event-ticket',
    'Event ticket',
    'A confirmed ticket to the event. Earn 40 approved hours to claim it.',
    2400
)
ON CONFLICT (slug) DO NOTHING;

CREATE TABLE shop_purchases (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    item_id UUID NOT NULL REFERENCES shop_items(id) ON DELETE RESTRICT,
    price_minutes BIGINT NOT NULL CHECK (price_minutes > 0),
    idempotency_key TEXT NOT NULL CHECK (char_length(idempotency_key) BETWEEN 16 AND 128),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, item_id),
    UNIQUE (user_id, idempotency_key)
);
CREATE INDEX shop_purchases_user_id_created_at_idx ON shop_purchases(user_id, created_at DESC);

-- The email is recorded with the purchase before the transaction commits. A
-- transient email-provider failure therefore cannot invalidate the purchase or
-- cause the confirmation intent to disappear.
CREATE TABLE notification_outbox (
    id UUID PRIMARY KEY,
    purchase_id UUID NOT NULL UNIQUE REFERENCES shop_purchases(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('ticket_purchase_confirmation')),
    sent_at TIMESTAMPTZ,
    processing_at TIMESTAMPTZ,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO user_wallets (user_id)
SELECT id FROM users
ON CONFLICT (user_id) DO NOTHING;
