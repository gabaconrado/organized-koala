-- Accounts. Auth is local-only (hard-constraint #5): username/email + argon2 hash.
-- Username and email are each globally unique; usernames may not contain '@'
-- (enforced server-side) so the login identifier space is collision-free.
CREATE TABLE users (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    username      TEXT        NOT NULL UNIQUE,
    email         TEXT        NOT NULL UNIQUE,
    password_hash TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
