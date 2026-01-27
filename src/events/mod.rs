use std::{
    io::ErrorKind,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use event_listener::Listener;
use inotify::{EventMask, Inotify, WatchMask};
use tokio::task::JoinHandle;
use zbus::Connection;

use crate::{
    config::{ConfigArgs, GRUB_ENV_PATH, GRUB_ROOT_PATH},
    dbus::connection::BootKitConfigSignals,
    dctx,
    errors::{DRes, DResult},
};

type EventHandle<T> = JoinHandle<DResult<T>>;

#[derive(Clone)]
pub struct BootkitEvents {
    connection: Connection,
    shutdown: Arc<AtomicBool>,
}

impl BootkitEvents {
    pub fn new(connection: &Connection) -> Self {
        Self {
            connection: connection.clone(),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Signal that all the listen_ functions should stop execution
    /// at the next available moment
    ///
    /// This method needs to take ownership to make sure `connection` is dropped
    /// after this call so `connection.graceful_shutdown()` doesn't hang
    pub fn signal_shutdown(self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    async fn listen_files_loop(&self) -> zbus::Result<()> {
        let mut inotify = Inotify::init().expect("Failed to initialize inotify");

        inotify
            .watches()
            .add(GRUB_ROOT_PATH, WatchMask::MODIFY)
            .expect("Failed to watch /etc/default/grub");
        inotify
            .watches()
            .add("/boot/grub2", WatchMask::MODIFY)
            .expect("Failed to watch /boot/grub2");

        log::info!("Listening to config changes");

        while !self.shutdown.load(Ordering::Relaxed) {
            let mut buffer = [0; 4096];

            let events = match inotify.read_events(&mut buffer) {
                Ok(events) => events,
                Err(error) if error.kind() == ErrorKind::WouldBlock => continue,
                Err(err) => panic!("Error while reading events: {err}"),
            };

            // prevent duplicate modify event triggers
            let mut signaled = false;
            for event in events {
                if event.mask.contains(EventMask::MODIFY)
                    && !signaled
                    && event.name.is_some_and(|name| name == "grub" || name == "grubenv")
                {
                    signaled = true;
                    self.connection
                        .object_server()
                        .interface("/org/opensuse/bootkit")
                        .await?
                        .file_changed()
                        .await?;
                    log::debug!("{GRUB_ROOT_PATH} contents was modified. Signaling dbus");
                    match event.name.unwrap().to_str().unwrap() {
                        "grub" => log::debug!("{GRUB_ROOT_PATH} contents was modified. Signaling dbus"),
                        "grubenv" => log::debug!("{GRUB_ENV_PATH} contents was modified. Signaling dbus"),
                        _ => {},
                    }
                }
            }
        }

        Ok(())
    }

    fn listen_files(&self) -> EventHandle<()> {
        let copy = self.clone();
        tokio::spawn(async move {
            copy.listen_files_loop()
                .await
                .ctx(dctx!(), "Failed to listen file events")
        })
    }

    fn detect_idle_connection(&self, timeout: Option<u64>) -> EventHandle<()> {
        let copy = self.clone();
        tokio::spawn(async move {
            // if timeout is not defined, there's no need to run the idle connection
            let timeout = if let Some(timeout) = timeout {
                timeout
            } else {
                return Ok(());
            };

            let mut counter = 0;

            while counter < timeout && !copy.shutdown.load(Ordering::Relaxed) {
                let activity = copy
                    .connection
                    .monitor_activity()
                    .wait_timeout(Duration::from_millis(100));
                if activity.is_none() {
                    counter += 100;
                } else {
                    counter = 0;
                }
            }

            // TODO: when this happens, send a signal to clients
            log::debug!("Idle counter limit exceeded. Stopping the program");
            Ok(())
        })
    }

    pub async fn listen_events(&self, config: &ConfigArgs) -> DResult<()> {
        let file_changes = self.listen_files();
        let idle_connection = self.detect_idle_connection(config.allowed_idle_time());
        let res = tokio::select! {
           res = file_changes => {
               res.ctx(dctx!(), "File change detection panicked")
           }
           res = idle_connection, if config.allowed_idle_time().is_some() => {
               res.ctx(dctx!(), "Idle detection panicked")
           }
        };

        res?
    }
}
