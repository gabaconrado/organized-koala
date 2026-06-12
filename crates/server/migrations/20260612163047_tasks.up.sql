-- TODO tasks, the flat shape of hard-constraint #3:
-- {title, description, status, created_at, closed_at}. No subtasks/tags/categories.
-- Each task is scoped to exactly one profile; deleting the profile removes its tasks.
-- `status` is constrained to the two ADR-0005 values; `closed_at` is set iff done.
CREATE TABLE tasks (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id  UUID        NOT NULL REFERENCES profiles (id) ON DELETE CASCADE,
    title       TEXT        NOT NULL,
    description TEXT        NOT NULL DEFAULT '',
    status      TEXT        NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'done')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at   TIMESTAMPTZ
);

CREATE INDEX tasks_profile_id_created_at_idx ON tasks (profile_id, created_at DESC);
