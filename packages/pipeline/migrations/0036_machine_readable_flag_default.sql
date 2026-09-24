-- The 0012 seed stored `panel.machine_readable` as off, while both the editor
-- frontend and editor-api register it with default on. A stored row beats the
-- default, so every database that ran the seed hid the Machine pane without
-- anyone having chosen that.
--
-- Only the untouched seed row is corrected. A row someone toggled is a choice
-- and stays as it is: the `update_updated_at` trigger moves `updated_at` on
-- every update, so a row whose `updated_at` still equals `created_at` was never
-- written after the seed inserted it (both columns got the same transaction
-- `now()`). The seed's description pins it further: a row created by a PUT
-- carries no description.
UPDATE feature_flags
SET enabled = true
WHERE key = 'panel.machine_readable'
  AND enabled = false
  AND updated_at = created_at
  AND description = 'Machine readable weergave';
