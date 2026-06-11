-- Id of selected systemd_boot snapshot, null if none is selected.
-- If none is selected, it implies that latest snapshot is being used.
ALTER TABLE selected_snapshot ADD COLUMN systemd_boot_snapshot_id INTEGER;

-- The database always has a single value that defaults to null
-- so it's fine to set it as such when the DB is defined
INSERT INTO selected_snapshot (systemd_boot_snapshot_id) VALUES (NULL);
