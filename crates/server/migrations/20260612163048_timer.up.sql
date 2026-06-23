-- Pomodoro timer: account-global duration config + at-most-one active focus session,
-- keyed on the user (ADR-0002 §5). The timer is NOT profile-scoped (#4 namespaces TODOs and
-- Notes only). The only knob is the duration (hard-constraint #3).
-- `user_id` PRIMARY KEY enforces at-most-one config / one active session per account in the
-- schema, so there is no app-level race. `ends_at` is derived (started_at + duration_minutes),
-- never stored, so the absolute end-instant has a single source of truth.
CREATE TABLE timer_configs (
    user_id          UUID        PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    duration_minutes INT         NOT NULL DEFAULT 30 CHECK (duration_minutes >= 1),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE timer_sessions (
    user_id          UUID        PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    started_at       TIMESTAMPTZ NOT NULL,
    duration_minutes INT         NOT NULL CHECK (duration_minutes >= 1)
);
