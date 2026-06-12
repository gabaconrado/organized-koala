-- Profiles are namespaces (hard-constraint #4): every task/note is scoped to one.
-- A profile belongs to exactly one user; deleting the user removes their profiles.
-- Each account is bootstrapped with one default profile at registration time.
CREATE TABLE profiles (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX profiles_user_id_idx ON profiles (user_id);
