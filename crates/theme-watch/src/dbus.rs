#[cfg(test)]
use crate::ThemeListener;
use crate::{ThemeWatchError, INTERFACE, OBJECT_PATH, SIGNAL};
use dbus::blocking::Connection;
use dbus::channel::Sender;
use dbus::message::{MatchRule, Message};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[cfg(test)]
use std::time::Duration;

pub(crate) fn connect(dirty: Arc<AtomicBool>) -> Option<Connection> {
    let connection = Connection::new_session().ok()?;
    let reload = dirty.clone();
    connection
        .add_match(
            MatchRule::new_signal(INTERFACE, SIGNAL).with_path(OBJECT_PATH),
            move |_: (), _, _| {
                reload.store(true, Ordering::Release);
                true
            },
        )
        .ok()?;
    let settings = dirty.clone();
    connection
        .add_match(
            MatchRule::new_signal("org.freedesktop.portal.Settings", "SettingChanged")
                .with_path("/org/freedesktop/portal/desktop")
                .with_sender("org.freedesktop.portal.Desktop"),
            move |(namespace, _, _): (
                String,
                String,
                dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>,
            ),
                  _,
                  _| {
                if matches!(
                    namespace.as_str(),
                    "org.freedesktop.appearance" | "org.creamui.appearance"
                ) {
                    settings.store(true, Ordering::Release);
                }
                true
            },
        )
        .ok()?;
    connection
        .add_match(
            MatchRule::new_signal("org.freedesktop.DBus", "NameOwnerChanged")
                .with_sender("org.freedesktop.DBus"),
            move |(name, _, _): (String, String, String), _, _| {
                if name == "org.freedesktop.portal.Desktop" {
                    dirty.store(true, Ordering::Release);
                }
                true
            },
        )
        .ok()?;
    Some(connection)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::{Mutex, MutexGuard};
    use std::time::Instant;

    #[test]
    fn portal_queries_and_live_reload_signals_preserve_provider_variants() {
        use creamui_appearance::{AppearanceSource, SystemColorScheme};
        use dbus::{
            arg::{PropMap, Variant},
            channel::MatchingReceiver,
        };
        use std::collections::HashMap;
        let _lock = bus_lock();
        let Some(_bus) = PrivateBus::start() else {
            return;
        };
        let portal = Connection::new_session().unwrap();
        portal
            .request_name("org.freedesktop.portal.Desktop", false, true, false)
            .unwrap();
        let mode = Arc::new(std::sync::atomic::AtomicU32::new(2));
        let current_mode = mode.clone();
        portal.start_receive(
            MatchRule::new_method_call().with_path("/org/freedesktop/portal/desktop"),
            Box::new(move |message, connection| {
                if message.member().as_deref() == Some("ReadAll") {
                    let mode = current_mode.load(Ordering::Acquire);
                    let mut standard = PropMap::new();
                    standard.insert("color-scheme".into(), Variant(Box::new(mode)));
                    standard.insert(
                        "accent-color".into(),
                        Variant(Box::new((0.25f64, 0.5f64, 1.0f64))),
                    );
                    let mut extension = PropMap::new();
                    if mode == 1 {
                        extension.insert(
                            "variant".into(),
                            Variant(Box::new(String::from("midnight"))),
                        );
                    }
                    let settings = HashMap::from([
                        (String::from("org.freedesktop.appearance"), standard),
                        (String::from("org.creamui.appearance"), extension),
                    ]);
                    connection
                        .send(message.method_return().append1(settings))
                        .unwrap();
                }
                true
            }),
        );
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let changed_mode = mode.clone();
        let worker = std::thread::spawn(move || {
            let mut last = 2;
            while !stopped.load(Ordering::Acquire) {
                portal.process(Duration::from_millis(10)).unwrap();
                let current = changed_mode.load(Ordering::Acquire);
                if current != last {
                    let message = Message::new_signal(
                        "/org/freedesktop/portal/desktop",
                        "org.freedesktop.portal.Settings",
                        "SettingChanged",
                    )
                    .unwrap()
                    .append3(
                        "org.freedesktop.appearance",
                        "color-scheme",
                        Variant(current),
                    );
                    portal.send(message).unwrap();
                    last = current;
                }
            }
        });
        // Snapshot metadata is tested separately; use a clean session source.
        let previous = std::env::var_os("CREAMUI_APPEARANCE");
        unsafe { std::env::remove_var("CREAMUI_APPEARANCE") };
        let listener = ThemeListener::start();
        assert!(listener.recv_timeout(Duration::from_secs(3)));
        let initial = creamui_appearance::query();
        assert_eq!(initial.source, AppearanceSource::Portal);
        assert_eq!(initial.color_scheme, SystemColorScheme::Light);
        assert_eq!(initial.variant, None);
        assert_eq!(
            initial.accent.map(|c| (c.r, c.g, c.b)),
            Some((64, 128, 255))
        );
        mode.store(1, Ordering::Release);
        let deadline = Instant::now() + Duration::from_millis(1500);
        loop {
            if listener.recv_timeout(Duration::from_millis(250)) {
                let updated = creamui_appearance::query();
                if updated.variant.as_deref() == Some("midnight") {
                    assert_eq!(updated.color_scheme, SystemColorScheme::Dark);
                    break;
                }
            }
            assert!(
                Instant::now() < deadline,
                "listener did not observe the host change"
            );
        }
        while listener.recv_timeout(Duration::from_millis(20)) {}
        publish().expect("publish custom theme reload");
        assert!(
            listener.recv_timeout(Duration::from_millis(1500)),
            "custom ReloadTheme did not wake the listener"
        );
        drop(listener);
        stop.store(true, Ordering::Release);
        worker.join().unwrap();
        unsafe {
            match previous {
                Some(value) => std::env::set_var("CREAMUI_APPEARANCE", value),
                None => std::env::remove_var("CREAMUI_APPEARANCE"),
            }
        }
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
