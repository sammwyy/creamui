use super::*;
use ::windows::{
    core::{w, IInspectable},
    Foundation::{EventRegistrationToken, TypedEventHandler},
    Win32::{
        System::{
            Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
            WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
        },
        UI::{
            Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
            WindowsAndMessaging::{
                SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
            },
        },
    },
    UI::ViewManagement::{UIColorType, UISettings},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// Balance initialization on this thread, including when a caller already has
// an STA apartment. Never uninitialize an apartment owned by the caller.
struct Apartment(bool);
impl Apartment {
    fn enter() -> Self {
        Self(unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok())
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { RoUninitialize() }
        }
    }
}

pub(super) fn query() -> SystemAppearance {
    let _apartment = Apartment::enter();
    let mut appearance = SystemAppearance::default();
    if let Ok(settings) = UISettings::new() {
        if let Ok(color) = settings.GetColorValue(UIColorType::Foreground) {
            appearance.color_scheme = scheme_from_foreground(color.R, color.G, color.B);
            appearance.source = AppearanceSource::Windows;
        }
        appearance.accent = settings
            .GetColorValue(UIColorType::Accent)
            .ok()
            .map(|c| Color::rgba(c.R, c.G, c.B, c.A));
        appearance.reduced_motion = settings.AnimationsEnabled().map(|v| !v).unwrap_or(false);
    }
    if appearance.source == AppearanceSource::Unavailable {
        let mut value = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let read = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("AppsUseLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some((&mut value as *mut u32).cast()),
                Some(&mut size),
            )
        };
        if read.is_ok() {
            appearance.color_scheme = if value == 0 {
                SystemColorScheme::Dark
            } else {
                SystemColorScheme::Light
            };
            appearance.source = AppearanceSource::Windows;
        }
    }
    let mut contrast = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    if unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            contrast.cbSize,
            Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .is_ok()
    {
        appearance.high_contrast = contrast.dwFlags.contains(HCF_HIGHCONTRASTON);
    }
    appearance
}

/// Owned by the watcher thread. UISettings delivers events independently of
/// the renderer; a periodic query also covers accessibility changes.
pub struct NativeSubscription {
    settings: UISettings,
    token: EventRegistrationToken,
    _apartment: Apartment,
}
impl NativeSubscription {
    pub fn start(dirty: Arc<AtomicBool>) -> Option<Self> {
        let apartment = Apartment::enter();
        let settings = UISettings::new().ok()?;
        let handler = TypedEventHandler::<UISettings, IInspectable>::new(move |_, _| {
            dirty.store(true, Ordering::Release);
            Ok(())
        });
        let token = settings.ColorValuesChanged(&handler).ok()?;
        Some(Self {
            settings,
            token,
            _apartment: apartment,
        })
    }
}
impl Drop for NativeSubscription {
    fn drop(&mut self) {
        let _ = self.settings.RemoveColorValuesChanged(self.token);
    }
}
