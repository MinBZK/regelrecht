-- Index the first 8 hex chars of `trajects.id` so `resolve_traject_ref`
-- in editor-api can look up trajects by their URL-form short id without
-- a sequential scan.
--
-- Every request to `/api/trajects/{ref}/corpus/...` runs the lookup —
-- without this index the planner falls back to a seq scan because the
-- comparison is on a derived expression (`left(id::text, 8)`) and can't
-- use the primary-key btree on `id`. A regular btree on the function
-- expression covers the equality lookup `resolve_traject_ref` uses.
CREATE INDEX IF NOT EXISTS trajects_short_id_idx
    ON trajects (left(id::text, 8));
