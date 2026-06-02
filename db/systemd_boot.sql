CREATE TABLE systemd_boot_snapshot (
    -- Auto incrementing snapshot id
    id INTEGER PRIMARY KEY NOT NULL,
    -- /boot/efi/loader/loader.conf config
    loader_config TEXT NOT NULL,
    -- selected kernel that's booted to, if it's actually specified
    selected_kernel TEXT,
    -- when snapshot was created
    created DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL
);
