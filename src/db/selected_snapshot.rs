use serde::Serialize;

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct SelectedSnapshot {
    /// Id of selected grub2 snapshot, null if none is selected.
    /// If none is selected, it implies that latest snapshot is being used.
    pub grub2_snapshot_id: Option<i64>,
    /// Id of selected systemd-boot snapshot, null if none is selected.
    /// If none is selected, it implies that latest snapshot is being used.
    pub systemd_boot_snapshot_id: Option<i64>,
}
