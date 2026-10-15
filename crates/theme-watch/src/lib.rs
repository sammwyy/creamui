//! Tells a running application when the persisted CreamUI appearance changed.
//!
//! Call [`ThemeListener::start`] and wait on [`ThemeListener::recv`]. The crate
//! selects the backend: Linux builds with the `dbus` feature subscribe to the
//! session bus, and every other build keeps an idle listener that never wakes.
//! `publish` announces a change after the appearance file has been saved.
//!
//! The `dbus` feature is on by default and links the D-Bus client only on
//! Linux. Windows, Android, and WebAssembly ignore it. A Linux build leaves
//! the client out with `default-features = false`.

use std::sync::mpsc;
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
}

impl ThemeListener {
    /// Starts the backend selected for this build and returns immediately.
    ///
    /// Linux with the `dbus` feature follows [`SIGNAL`] on the session bus.
    /// Every other build stays pending, so the startup call is the same on
    /// every platform.
    pub fn start() -> Self {
        #[cfg(all(feature = "dbus", target_os = "linux"))]
        {
            dbus::start()
        }
        #[cfg(not(all(feature = "dbus", target_os = "linux")))]
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
#[cfg(any(test, not(all(feature = "dbus", target_os = "linux"))))]
fn idle_listener() -> ThemeListener {
    let (sender, events) = mpsc::channel();
    static HOLD: std::sync::Mutex<Vec<mpsc::Sender<()>>> = std::sync::Mutex::new(Vec::new());
    if let Ok(mut hold) = HOLD.lock() {
        hold.push(sender);
    }
    ThemeListener { events }
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
