-- Notes, the flat shape of hard-constraint #3: {title, content, created_at}. No folders/tags.
-- Each note is scoped to exactly one profile (#4 namespaces TODOs and Notes); deleting the
-- profile removes its notes via the FK cascade (pre-wires the item 0012 namespace delete).
-- Only `created_at` — there is NO `updated_at` (#3, operator-locked); edits mutate in place.
CREATE TABLE notes (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id  UUID        NOT NULL REFERENCES profiles (id) ON DELETE CASCADE,
    title       TEXT        NOT NULL,
    content     TEXT        NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX notes_profile_id_created_at_idx ON notes (profile_id, created_at DESC);
