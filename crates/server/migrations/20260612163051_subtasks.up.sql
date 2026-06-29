-- Sub-tasks, the bounded one-level child of a task (ADR-0012/ADR-0013): {title, status} only.
-- A sub-task references its parent task, never another sub-task, so the schema structurally
-- enforces a single level of nesting (no `parent_subtask_id`). Profile-scoping is inherited
-- via the parent task (no `profile_id` column); every query joins `subtasks → tasks`.
-- `ON DELETE CASCADE` to `tasks` is the entire no-orphans guarantee: deleting a task removes
-- its sub-tasks, and deleting a profile transitively cascades through `tasks` to `subtasks`.
-- `created_at` exists only to give a stable creation-order sort; it is NOT exposed on the wire.
CREATE TABLE subtasks (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id    UUID        NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
    title      TEXT        NOT NULL,
    status     TEXT        NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'done')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX subtasks_task_id_created_at_idx ON subtasks (task_id, created_at);
