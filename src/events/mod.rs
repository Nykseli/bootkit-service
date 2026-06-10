use std::{
    collections::HashMap,
    io::ErrorKind,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use event_listener::Listener;
use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use zbus::Connection;

use crate::{
    bootloader::{systemd_boot::boot_entries::SystemdBootEntries, BootloaderType},
    config::{ConfigArgs, GRUB_BOOT_PATH, GRUB_ROOT_PATH, SYSTEMD_CFG_ROOT},
    dbus::connection::BootKitConfigSignals,
    dctx,
    errors::{DError, DRes, DResult},
};

type EventHandle<T> = JoinHandle<DResult<T>>;

#[derive(Debug)]
pub struct EventWatchDir {
    dir: String,
    files: Vec<String>,
}

impl EventWatchDir {
    fn new<D: Into<String>, F: Into<Vec<String>>>(dir: D, files: F) -> Self {
        Self {
            dir: dir.into(),
            files: files.into(),
        }
    }

    fn find_full_path(&self, name: &str) -> Option<String> {
        let found = self.files.iter().any(|file| file.as_str() == name);

        if found {
            Some(format!("{}/{}", self.dir, name))
        } else {
            None
        }
    }

    fn grub2_watch_dirs() -> Vec<Self> {
        vec![
            Self::new(GRUB_ROOT_PATH, ["grub".to_string()]),
            Self::new(GRUB_BOOT_PATH, ["grubenv".to_string()]),
        ]
    }

    fn systemd_boot_watch_dirs() -> DResult<Vec<Self>> {
        let entries =
            SystemdBootEntries::new().ctx(dctx!(), "Failed to get systemd-boot entries")?;
        let entry_ids: Vec<String> = entries
            .entry_files()
            .iter()
            .map(|entry| entry.id().to_string())
            .collect();

        Ok(vec![
            Self::new(SYSTEMD_CFG_ROOT, ["loader.conf".to_string()]),
            Self::new("/boot/efi/loader/entries", entry_ids),
        ])
    }

    fn system_watch_dirs() -> DResult<Vec<Self>> {
        match BootloaderType::system_type() {
            BootloaderType::Grub => Ok(Self::grub2_watch_dirs()),
            BootloaderType::SystemdBoot => Self::systemd_boot_watch_dirs()
                .ctx(dctx!(), "Failed to get watch files for systemd-boot"),
        }
    }
}

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

    async fn listen_files_loop(&self) -> DResult<()> {
        let watch_dirs = EventWatchDir::system_watch_dirs()
            .ctx(dctx!(), "Failed to get system watch directories")?;
        let mut notify_watch: HashMap<WatchDescriptor, EventWatchDir> = HashMap::new();
        let mut inotify = Inotify::init().ctx(dctx!(), "Failed to initialize inotify")?;

        for watch in watch_dirs {
            let wd = inotify
                .watches()
                .add(&watch.dir, WatchMask::MODIFY)
                .unwrap_or_else(|_| panic!("Failed to watch {}", &watch.dir));
            notify_watch.insert(wd, watch);
        }

        while !self.shutdown.load(Ordering::Relaxed) {
            let mut buffer = [0; 4096];

            let events = match inotify.read_events(&mut buffer) {
                Ok(events) => events,
                Err(error) if error.kind() == ErrorKind::WouldBlock => continue,
                Err(err) => {
                    return Err(DError::generic(
                        dctx!(),
                        format!("Error while reading events: {err}"),
                    ))
                }
            };

            // prevent duplicate modify event triggers
            let mut signaled = false;
            for event in events {
                if !event.mask.contains(EventMask::MODIFY) || signaled {
                    continue;
                }

                let event_watch = notify_watch
                    .get(&event.wd)
                    .ctx(dctx!(), "Couldn't find notify files")?;

                let file_match = event
                    .name
                    .and_then(|name| name.to_str())
                    .and_then(|name| event_watch.find_full_path(name));

                if let Some(file) = file_match {
                    signaled = true;
                    self.connection
                        .object_server()
                        .interface("/org/opensuse/bootkit")
                        .await
                        .ctx(dctx!(), "Failed to get dbus interface")?
                        .file_changed()
                        .await
                        .ctx(dctx!(), "Failed to call file_chaned")?;

                    log::debug!("{file} contents was modified. Signaling dbus")
                }
            }

            // XXX: could we use epoll instead of crude timeouts?
            thread::sleep(Duration::from_millis(100));
        }

        Ok(())
    }

    fn listen_files(&self) -> EventHandle<()> {
        let copy = self.clone();
        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .worker_threads(1)
                .build()
                .unwrap();

            rt.block_on(async {
                copy.listen_files_loop()
                    .await
                    .ctx(dctx!(), "Failed to listen file modifications")
            })
        })
    }

    fn detect_idle_connection(&self, timeout: Option<u64>) -> EventHandle<()> {
        let copy = self.clone();
        thread::spawn(move || {
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
        loop {
            if file_changes.is_finished() {
                return file_changes
                    .join()
                    .ctx(dctx!(), "File change detection panicked")?;
            }
            if idle_connection.is_finished() {
                return idle_connection
                    .join()
                    .ctx(dctx!(), "Idle detection panicked")?;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}
