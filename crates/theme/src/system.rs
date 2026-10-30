//! The host's appearance preference, independent of an app's selected theme.
use crate::Color;
use creamui_reactive::Signal;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SystemColorScheme {
    #[default]
    NoPreference,
    Light,
    Dark,
}

impl SystemColorScheme {
    pub fn from_portal(value: u32) -> Self {
        match value {
            1 => Self::Dark,
            2 => Self::Light,
            _ => Self::NoPreference,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoPreference => "default",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppearanceSource {
    #[default]
    Unavailable,
    Portal,
    GSettings,
    Windows,
    DesktopSnapshot,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemAppearance {
    pub color_scheme: SystemColorScheme,
    /// Provider-defined identifier, e.g. "midnight" or "mocha". A standard
    /// dark preference alone never implies a particular named variant.
    pub variant: Option<String>,
    pub accent: Option<Color>,
    pub high_contrast: bool,
    pub reduced_motion: bool,
    pub source: AppearanceSource,
}

/// One provider shared by every window in an application. The renderer
/// updates it on the UI thread; observing widgets subscribe through Signal.
#[derive(Clone)]
pub struct SystemAppearanceProvider(Signal<SystemAppearance>);
impl SystemAppearanceProvider {
    pub fn new(appearance: SystemAppearance) -> Self {
        Self(Signal::new(appearance))
    }
    pub fn signal(&self) -> Signal<SystemAppearance> {
        self.0.clone()
    }
    pub fn get(&self) -> SystemAppearance {
        self.0.get()
    }
    pub fn set(&self, appearance: SystemAppearance) {
        if self.0.peek() != appearance {
            self.0.set(appearance);
        }
    }
}
impl Default for SystemAppearanceProvider {
    fn default() -> Self {
        Self::new(SystemAppearance::default())
    }
}

/// Reactive host preference. Available inside a render window's UI scope
/// with the `system-theme` feature, including windows with a fixed app theme.
pub fn use_system_appearance() -> Signal<SystemAppearance> {
    creamui_reactive::use_context::<SystemAppearanceProvider>().signal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    #[test]
    fn host_hook_is_reactive_and_keeps_the_provider_variant() {
        creamui_reactive::with_context_scope(|| {
            let provider = SystemAppearanceProvider::default();
            creamui_reactive::provide_context(provider.clone());
            let seen = Rc::new(RefCell::new(Vec::new()));
            let output = seen.clone();
            let _effect = creamui_reactive::create_effect(move || {
                output.borrow_mut().push(use_system_appearance().get())
            });
            let host = SystemAppearance {
                color_scheme: SystemColorScheme::Dark,
                variant: Some("midnight".into()),
                ..Default::default()
            };
            provider.set(host.clone());
            provider.set(host.clone());
            assert_eq!(seen.borrow().len(), 2);
            assert_eq!(seen.borrow()[1], host);
        });
    }
}
