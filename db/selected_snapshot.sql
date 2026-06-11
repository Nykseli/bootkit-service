CREATE TABLE selected_snapshot (
    -- Id of selected grub2 snapshot, null if none is selected.
    -- If none is selected, it implies that latest snapshot is being used.
    grub2_snapshot_id INTEGER
);

-- The database always has a single value that defaults to null
-- so it's fine to set it as such when the DB is defined
INSERT INTO selected_snapshot (grub2_snapshot_id) VALUES (NULL);
