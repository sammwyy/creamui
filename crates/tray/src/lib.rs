//! Native system-tray abstraction used by CreamUI's desktop runtime.
//!
//! The public API is intentionally UI-framework agnostic: backends emit a
//! [`TrayEvent`] containing a menu id, and the embedding runtime decides how
//! that should affect its own state. This keeps the platform service thread
//! separate from non-`Send` UI state such as CreamUI signals.

use std::fmt;
use std::sync::Arc;

/// Raw RGBA image data for a system-tray icon.
#[derive(Clone)]
pub struct TrayIcon {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

/// Returned when raw icon pixels do not match the given dimensions.
#[derive(Debug, Clone, Copy)]
pub struct InvalidTrayIcon;

impl fmt::Display for InvalidTrayIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("tray icon data must contain exactly width × height × 4 RGBA bytes")
    }
}

impl std::error::Error for InvalidTrayIcon {}

impl TrayIcon {
    /// Validates raw RGBA icon pixels. A 24–32px square icon is usually a
    /// good fit for a desktop panel.
    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, InvalidTrayIcon> {
        if rgba.len() != width as usize * height as usize * 4 {
            return Err(InvalidTrayIcon);
        }
        Ok(Self {
            rgba,
            width,
            height,
        })
    }
}

/// A menu activation emitted by the platform tray backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayEvent {
    /// The id supplied to [`TrayBuilder::item`].
    pub id: String,
}

/// An error returned while registering a native system tray.
#[derive(Debug)]
pub enum TrayError {
    /// The selected target has no enabled native backend yet.
    UnsupportedPlatform,
    /// A platform backend rejected tray registration.
    Backend(String),
}

impl fmt::Display for TrayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrayError::UnsupportedPlatform => formatter.write_str(
                "CreamUI system tray has no enabled backend for this target; enable a platform backend feature",
            ),
            TrayError::Backend(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for TrayError {}

/// Declarative system-tray menu definition.
pub struct TrayBuilder {
    id: String,
    icon: TrayIcon,
    title: String,
    items: Vec<MenuItem>,
}

struct MenuItem {
    id: String,
    label: String,
    enabled: bool,
}

impl TrayBuilder {
    /// Starts a tray definition with its icon.
    pub fn new(icon: TrayIcon) -> Self {
        Self {
            id: "creamui-tray".into(),
            icon,
            title: String::new(),
            items: Vec::new(),
        }
    }

    /// Gives this tray a process-unique native id.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Sets the title/tooltip where the desktop exposes one.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Adds an enabled context-menu item.
    pub fn item(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.items.push(MenuItem {
            id: id.into(),
            label: label.into(),
            enabled: true,
        });
        self
    }

    /// Adds a disabled context-menu item.
    pub fn disabled_item(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.items.push(MenuItem {
            id: id.into(),
            label: label.into(),
            enabled: false,
        });
        self
    }

    /// Registers the platform service. Keep the returned [`Tray`] alive for
    /// as long as the icon should remain visible.
    pub fn build(
        self,
        on_event: impl Fn(TrayEvent) + Send + Sync + 'static,
    ) -> Result<Tray, TrayError> {
        let on_event = Arc::new(on_event);
        #[cfg(all(target_os = "linux", feature = "linux-status-notifier"))]
        {
            return linux::build(self, on_event);
        }
        #[allow(unreachable_code)]
        {
            let _ = on_event;
            Err(TrayError::UnsupportedPlatform)
        }
    }
}

/// A live native tray registration. Dropping it removes the icon.
pub struct Tray {
    #[cfg(all(target_os = "linux", feature = "linux-status-notifier"))]
    _backend: ksni::blocking::Handle<linux::LinuxTray>,
}

#[cfg(all(target_os = "linux", feature = "linux-status-notifier"))]
mod linux {
    use super::{MenuItem, Tray, TrayBuilder, TrayError, TrayEvent};
    use ksni::{blocking::TrayMethods, menu::StandardItem, MenuItem as KsniMenuItem};
    use std::sync::Arc;

    pub(super) struct LinuxTray {
        id: String,
        title: String,
        icon: ksni::Icon,
        items: Vec<MenuItem>,
        on_event: Arc<dyn Fn(TrayEvent) + Send + Sync>,
    }

    pub(super) fn build(
        builder: TrayBuilder,
        on_event: Arc<dyn Fn(TrayEvent) + Send + Sync>,
    ) -> Result<Tray, TrayError> {
        let icon = rgba_to_argb(&builder.icon.rgba);
        let service = LinuxTray {
            id: builder.id,
            title: builder.title,
            icon: ksni::Icon {
                width: builder.icon.width as i32,
                height: builder.icon.height as i32,
                data: icon,
            },
            items: builder.items,
            on_event,
        };
        let backend = service
            // Some desktops bring up their SNI watcher after autostart apps.
            // Keeping the service running lets it attach when that happens.
            .assume_sni_available(true)
            .spawn()
            .map_err(|error| {
                TrayError::Backend(format!(
                    "failed to create StatusNotifierItem tray: {error:?}"
                ))
            })?;
        Ok(Tray { _backend: backend })
    }

    impl ksni::Tray for LinuxTray {
        fn id(&self) -> String {
            self.id.clone()
        }

        fn title(&self) -> String {
            self.title.clone()
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            vec![self.icon.clone()]
        }

        fn menu(&self) -> Vec<KsniMenuItem<Self>> {
            self.items
                .iter()
                .map(|item| {
                    let id = item.id.clone();
                    let on_event = self.on_event.clone();
                    StandardItem {
                        label: item.label.clone(),
                        enabled: item.enabled,
                        activate: Box::new(move |_| on_event(TrayEvent { id: id.clone() })),
                        ..Default::default()
                    }
                    .into()
                })
                .collect()
        }
    }

    fn rgba_to_argb(rgba: &[u8]) -> Vec<u8> {
        let mut argb = Vec::with_capacity(rgba.len());
        for pixel in rgba.chunks_exact(4) {
            // StatusNotifierItem specifies ARGB in network byte order.
            argb.extend_from_slice(&[pixel[3], pixel[0], pixel[1], pixel[2]]);
        }
        argb
    }
}
