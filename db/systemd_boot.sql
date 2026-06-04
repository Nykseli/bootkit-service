CREATE TABLE systemd_boot_snapshot (
    -- Auto incrementing snapshot id
    id INTEGER PRIMARY KEY NOT NULL,
    -- /boot/efi/loader/loader.conf config
    loader_config TEXT NOT NULL,
    -- selected entry config name
    selected_entry TEXT NOT NULL,
    -- kernel args for the selected kernel
    -- systemd-boot ties kernel args to the boot entry
    kernel_arguments TEXT,
    -- /boot/efi/loader/entries/ config data
    entry_config TEXT NOT NULL,
    -- when snapshot was created
    created DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL
);
