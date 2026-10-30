//! Live host, session snapshot, and persisted CreamUI appearance notifications.
//! Linux follows the public Settings portal; Windows follows UISettings.
//! File checks and a bounded native query also cover missed notifications and
//! desktops without a portal. Dropping the listener stops its worker.

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc,
};
static REVISION: AtomicU64 = AtomicU64::new(0);
use std::time::Duration;

#[cfg(all(feature = "dbus", target_os = "linux"))]
mod dbus;

/// Object path of the CreamUI appearance signal.
pub const OBJECT_PATH: &str = "/org/creamui/Theme";
/// D-Bus interface of the CreamUI appearance signal.
pub const INTERFACE: &str = "org.creamui.Theme";
/// Signal member emitted after the system appearance is saved.
pub const SIGNAL: &str = "ReloadTheme";

/// Failure while announcing an appearance change.
#[derive(Debug)]
pub struct ThemeWatchError {
    message: String,
}

impl ThemeWatchError {
    #[cfg(all(feature = "dbus", target_os = "linux"))]
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ThemeWatchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ThemeWatchError {}

/// Receives requests to reload the persisted CreamUI appearance.
///
/// The listener is not `Sync`. Share it with a mutex when another thread waits
/// on it.
pub struct ThemeListener {
    events: mpsc::Receiver<()>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    _keepalive: Option<mpsc::Sender<()>>,
}
impl Drop for ThemeListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl ThemeListener {
    /// Subscribe immediately. Native workers are available on Linux and
    /// Windows; unsupported targets keep an idle listener. Native backends
    /// send an initial reconciliation event after subscribing.
    pub fn start() -> Self {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            start_native()
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            idle_listener()
        }
    }

    /// Blocks until the appearance should be reloaded.
    ///
    /// Returns `false` when the backend has stopped.
    pub fn recv(&self) -> bool {
        self.events.recv().is_ok()
    }

    /// Waits up to `timeout` for a reload request.
    ///
    /// Returns `false` on timeout and when the backend has stopped.
    pub fn recv_timeout(&self, timeout: Duration) -> bool {
        self.events.recv_timeout(timeout).is_ok()
    }
}

/// Announces that the persisted CreamUI appearance changed.
///
/// The idle backend reports success without sending anything.
pub fn publish() -> Result<(), ThemeWatchError> {
    REVISION.fetch_add(1, Ordering::Release);
    #[cfg(all(feature = "dbus", target_os = "linux"))]
    {
        dbus::publish()
    }
    #[cfg(not(all(feature = "dbus", target_os = "linux")))]
    {
        Ok(())
    }
}

/// Keeps the channel open so [`ThemeListener::recv`] waits instead of looking
/// like a backend that failed to start.
#[cfg(any(test, not(any(target_os = "linux", target_os = "windows"))))]
fn idle_listener() -> ThemeListener {
    let (sender, events) = mpsc::channel();
    ThemeListener {
        events,
        stop: Arc::default(),
        worker: None,
        _keepalive: Some(sender),
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn start_native() -> ThemeListener {
    let (sender, events) = mpsc::sync_channel(1);
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let worker = std::thread::Builder::new()
        .name("creamui-appearance".into())
        .spawn(move || {
            let dirty = Arc::new(AtomicBool::new(false));
            let _native = creamui_appearance::NativeSubscription::start(dirty.clone());
            #[cfg(all(feature = "dbus", target_os = "linux"))]
            let mut bus = dbus::connect(dirty.clone());
            let mut previous = creamui_appearance::query();
            let mut files = file_stamps();
            let mut revision = REVISION.load(Ordering::Acquire);
            let mut next_query = std::time::Instant::now() + Duration::from_secs(2);
            // Reconcile after subscription: startup resolution may have happened
            // immediately before the desktop changed or this worker connected.
            let _ = sender.try_send(());
            while !stopped.load(Ordering::Acquire) {
                #[cfg(all(feature = "dbus", target_os = "linux"))]
                if let Some(connection) = &bus {
                    if connection.process(Duration::from_millis(200)).is_err() {
                        bus = None;
                    }
                } else {
                    std::thread::sleep(Duration::from_millis(200));
                }
                #[cfg(not(all(feature = "dbus", target_os = "linux")))]
                std::thread::sleep(Duration::from_millis(200));
                let current_files = file_stamps();
                let current_revision = REVISION.load(Ordering::Acquire);
                let mut changed = current_files != files
                    || current_revision != revision
                    || dirty.swap(false, Ordering::AcqRel);
                files = current_files;
                revision = current_revision;
                if std::time::Instant::now() >= next_query {
                    #[cfg(all(feature = "dbus", target_os = "linux"))]
                    if bus.is_none() {
                        bus = dbus::connect(dirty.clone());
                    }
                    let current = creamui_appearance::query();
                    changed |= current != previous;
                    previous = current;
                    next_query = std::time::Instant::now() + Duration::from_secs(2);
                }
                if changed
                    && matches!(
                        sender.try_send(()),
                        Err(mpsc::TrySendError::Disconnected(_))
                    )
                {
                    break;
                }
            }
        })
        .map_err(|error| log::error!("failed to start appearance listener: {error}"))
        .ok();
    ThemeListener {
        events,
        stop,
        worker,
        _keepalive: None,
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn file_stamps() -> Vec<(std::path::PathBuf, Option<(std::time::SystemTime, u64)>)> {
    creamui_theme_loader::watch_paths()
        .into_iter()
        .map(|path| {
            let stamp = std::fs::metadata(&path)
                .ok()
                .and_then(|m| Some((m.modified().ok()?, m.len())));
            (path, stamp)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_listener_stays_pending() {
        let listener = idle_listener();
        assert!(matches!(
            listener.events.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(!listener.recv_timeout(Duration::from_millis(20)));
        assert!(matches!(
            listener.events.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[cfg(not(all(feature = "dbus", target_os = "linux")))]
    #[test]
    fn publish_on_the_idle_backend_succeeds() {
        assert!(publish().is_ok());
    }
}
