use crate::{ThemeListener, ThemeWatchError, INTERFACE, OBJECT_PATH, SIGNAL};
use dbus::blocking::Connection;
use dbus::channel::Sender;
use dbus::message::{MatchRule, Message};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub(crate) fn start() -> ThemeListener {
    let (sender, events) = mpsc::channel();
    if let Err(error) = thread::Builder::new()
        .name("creamui-theme-watch".into())
        .spawn(move || listen(sender))
    {
        log::error!("failed to start theme listener: {error}");
    }
    ThemeListener { events }
}

pub(crate) fn publish() -> Result<(), ThemeWatchError> {
    let connection = Connection::new_session()
        .map_err(|error| ThemeWatchError::new(format!("session bus unavailable: {error}")))?;
    let message = Message::new_signal(OBJECT_PATH, INTERFACE, SIGNAL)
        .map_err(|error| ThemeWatchError::new(error.to_string()))?;
    connection
        .send(message)
        .map(|_| ())
        .map_err(|_| ThemeWatchError::new("failed to send theme reload signal"))
}

fn listen(sender: mpsc::Sender<()>) {
    let Ok(connection) = Connection::new_session() else {
        log::warn!("session bus unavailable; live theme updates are disabled");
        return;
    };
    if let Err(error) = connection.add_match(
        MatchRule::new_signal(INTERFACE, SIGNAL).with_path(OBJECT_PATH),
        move |_: (), _, _| sender.send(()).is_ok(),
    ) {
        log::error!("failed to listen for theme updates: {error}");
        return;
    }
    loop {
        if let Err(error) = connection.process(Duration::from_secs(1)) {
            log::error!("theme listener stopped: {error}");
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::{Mutex, MutexGuard};
    use std::time::Instant;

    #[test]
    fn published_reload_wakes_the_listener() {
        let _lock = bus_lock();
        let Some(bus) = PrivateBus::start() else {
            eprintln!("creamui-theme-watch: skipping, dbus-daemon is unavailable");
            return;
        };
        let listener = ThemeListener::start();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            publish().expect("publish theme reload");
            if listener.recv_timeout(Duration::from_millis(100)) {
                return;
            }
        }
        drop(bus);
        panic!("listener did not observe ReloadTheme");
    }

    fn bus_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct PrivateBus {
        pid: i32,
        previous: Option<std::ffi::OsString>,
    }

    impl PrivateBus {
        fn start() -> Option<Self> {
            let launcher = Command::new("dbus-daemon")
                .args(["--session", "--fork", "--print-address=1", "--print-pid=1"])
                .output()
                .ok()?;
            if !launcher.status.success() {
                return None;
            }
            let text = String::from_utf8(launcher.stdout).ok()?;
            let mut lines = text.lines();
            let address = lines.next()?.to_owned();
            let pid = lines.next()?.parse().ok()?;
            if address.is_empty() {
                return None;
            }
            let previous = std::env::var_os("DBUS_SESSION_BUS_ADDRESS");
            // SAFETY: this test holds `bus_lock` for its whole body, sets the
            // address before any listener thread starts, and restores it in
            // Drop after the private daemon has been killed.
            unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address) };
            Some(Self { pid, previous })
        }
    }

    impl Drop for PrivateBus {
        fn drop(&mut self) {
            let _ = Command::new("kill").arg(self.pid.to_string()).status();
            unsafe { restore_bus(self.previous.take()) };
        }
    }

    unsafe fn restore_bus(previous: Option<std::ffi::OsString>) {
        match previous {
            Some(value) => std::env::set_var("DBUS_SESSION_BUS_ADDRESS", value),
            None => std::env::remove_var("DBUS_SESSION_BUS_ADDRESS"),
        }
    }
}
