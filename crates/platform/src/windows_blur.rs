//! Backdrop blur through DWM: Acrylic where the compositor supports system
//! backdrops (Windows 11 22H2+), the accent-policy blur-behind elsewhere
//! (Windows 10). The backdrop covers the whole window; `BlurRegion`
//! geometry has no Win32 equivalent. On Windows 10 the blur also covers
//! the invisible resize borders, so it protrudes ~7px past the visible
//! frame on the left, right and bottom edges.

use std::sync::OnceLock;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Dwm::{
    DwmEnableBlurBehindWindow, DwmSetWindowAttribute, DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW,
    DWMWA_SYSTEMBACKDROP_TYPE, DWM_BB_ENABLE, DWM_BLURBEHIND,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

const WCA_ACCENT_POLICY: u32 = 0x13;
const ACCENT_DISABLED: u32 = 0;
const ACCENT_ENABLE_BLURBEHIND: u32 = 3;

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct WindowCompositionAttributeData {
    attribute: u32,
    data: *mut AccentPolicy,
    size: usize,
}

type SetWindowCompositionAttributeFn =
    unsafe extern "system" fn(HWND, *mut WindowCompositionAttributeData) -> i32;

fn set_window_composition_attribute() -> Option<SetWindowCompositionAttributeFn> {
    static FUNCTION: OnceLock<Option<SetWindowCompositionAttributeFn>> = OnceLock::new();
    *FUNCTION.get_or_init(|| unsafe {
        let module = GetModuleHandleW(windows_sys::core::w!("user32.dll"));
        if module.is_null() {
            return None;
        }
        GetProcAddress(
            module,
            windows_sys::core::s!("SetWindowCompositionAttribute"),
        )
        .map(|pointer| std::mem::transmute(pointer))
    })
}

/// Blur through the undocumented accent policy, the only backdrop the
/// compositor offers on Windows 10 and Windows 11 before 22H2. Acrylic
/// (state 4) is avoided there: on Windows 10 it makes dragged windows
/// stick to the cursor.
fn set_accent_policy(hwnd: HWND, enabled: bool) -> bool {
    let Some(set_attribute) = set_window_composition_attribute() else {
        return false;
    };
    let mut policy = AccentPolicy {
        accent_state: if enabled {
            ACCENT_ENABLE_BLURBEHIND
        } else {
            ACCENT_DISABLED
        },
        accent_flags: if enabled { 2 } else { 0 },
        // ABGR: a faint dark tint over the blurred desktop.
        gradient_color: 0x3300_0000,
        animation_id: 0,
    };
    let mut data = WindowCompositionAttributeData {
        attribute: WCA_ACCENT_POLICY,
        data: &mut policy,
        size: std::mem::size_of::<AccentPolicy>(),
    };
    unsafe { set_attribute(hwnd, &mut data) != 0 }
}

pub(crate) fn set(hwnd: HWND, enabled: bool) {
    unsafe {
        let backdrop = if enabled {
            DWMSBT_TRANSIENTWINDOW
        } else {
            DWMSBT_NONE
        };
        if DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE as u32,
            &backdrop as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&backdrop) as u32,
        ) == windows_sys::Win32::Foundation::S_OK
        {
            return;
        }
        if set_accent_policy(hwnd, enabled) {
            return;
        }
        let blur = DWM_BLURBEHIND {
            dwFlags: DWM_BB_ENABLE,
            fEnable: if enabled { 1 } else { 0 },
            hRgnBlur: std::ptr::null_mut(),
            fTransitionOnMaximized: 0,
        };
        if DwmEnableBlurBehindWindow(hwnd, &blur) != windows_sys::Win32::Foundation::S_OK {
            log::debug!("creamui-platform: DWM rejected the window blur request");
        }
    }
}
