use inotify::{EventMask, Inotify, WatchMask};
use zbus::Connection;

use crate::{config::GRUB_ROOT_PATH, dbus::connection::BootKitConfigSignals};

pub async fn listen_files(connection: &Connection) -> zbus::Result<()> {
    let mut inotify = Inotify::init().expect("Failed to initialize inotify");
    inotify
        .watches()
        .add(GRUB_ROOT_PATH, WatchMask::MODIFY)
        .expect("Failed to watch /etc/default/grub");

    log::info!("Listening to config changes");

    loop {
        let mut buffer = [0; 4096];
        let events = inotify
            .read_events_blocking(&mut buffer)
            .expect("Failed to read inotify events");

        // prevent duplicate modify event triggers
        let mut signaled = false;
        for event in events {
            if event.mask.contains(EventMask::MODIFY) && !signaled {
                if event.name.is_some_and(|name| name == "grub") {
                    signaled = true;
                    connection
                        .object_server()
                        .interface("/org/opensuse/bootkit")
                        .await?
                        .file_changed()
                        .await?;
                    log::debug!("{GRUB_ROOT_PATH} contents was modified. Signaling dbus");
                }
            }
        }
    }
}
