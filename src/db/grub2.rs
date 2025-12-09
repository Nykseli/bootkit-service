use chrono::NaiveDateTime;

#[derive(Debug)]
#[allow(dead_code)]
pub struct Grub2Snapshot {
    /// Auto incrementing snapshot id
    pub id: i64,
    /// /etc/default/grub config
    pub grub_config: String,
    /// selected kernel that's booted to, if it's actually specified
    pub selected_kernel: Option<String>,
    /// when snapshot was created
    pub created: NaiveDateTime,
}
