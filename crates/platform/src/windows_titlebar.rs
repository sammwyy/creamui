//! Client-drawn titlebar on Win32 that keeps the native frame behaviour:
//! resize borders, the drop shadow, Aero Snap and the Windows 11 snap layouts
//! flyout on the maximize button.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
use windows_sys::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient};
use windows_sys::Win32::UI::Controls::MARGINS;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, IsZoomed, SetWindowPos, ShowWindow, HTCLIENT, HTMAXBUTTON, HTTOP,
    NCCALCSIZE_PARAMS, SM_CXPADDEDBORDER, SM_CYFRAME, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_MAXIMIZE, SW_RESTORE, WM_MOUSEMOVE, WM_NCCALCSIZE,
    WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE,
    WM_NCMOUSEMOVE,
};

/// Logical size of one caption button. Client titlebars draw their
/// minimize, maximize and close buttons flush with the top-right corner at
/// exactly this size so the hit-test below lines up with them.
pub const CAPTION_BUTTON_WIDTH: f64 = 46.0;
pub const CAPTION_BUTTON_HEIGHT: f64 = 32.0;

const SUBCLASS_ID: usize = 0x4352_4D55;

#[derive(Default)]
struct FrameState {
    maximize_hovered: AtomicBool,
    maximize_pressed: AtomicBool,
}

pub(crate) struct CustomFrame {
    hwnd: isize,
    state: Arc<FrameState>,
}

impl CustomFrame {
    pub(crate) fn install(hwnd: isize) -> Self {
        let state = Arc::new(FrameState::default());
        let window = hwnd as HWND;
        unsafe {
            if SetWindowSubclass(
                window,
                Some(subclass_proc),
                SUBCLASS_ID,
                Arc::as_ptr(&state) as usize,
            ) == 0
            {
                log::warn!("creamui-platform: could not subclass the window for a custom titlebar");
            }
            extend_frame(window, 1);
            refresh_frame(window);
        }
        Self { hwnd, state }
    }

    pub(crate) fn maximize_hovered(&self) -> bool {
        self.state.maximize_hovered.load(Ordering::Relaxed)
    }
}

impl Drop for CustomFrame {
    fn drop(&mut self) {
        let window = self.hwnd as HWND;
        unsafe {
            RemoveWindowSubclass(window, Some(subclass_proc), SUBCLASS_ID);
            extend_frame(window, 0);
            refresh_frame(window);
        }
    }
}

/// A one-pixel extension keeps DWM drawing the window shadow once the
/// caption is gone.
unsafe fn extend_frame(window: HWND, top: i32) {
    let margins = MARGINS {
        cxLeftWidth: 0,
        cxRightWidth: 0,
        cyTopHeight: top,
        cyBottomHeight: 0,
    };
    DwmExtendFrameIntoClientArea(window, &margins);
}

unsafe fn refresh_frame(window: HWND) {
    SetWindowPos(
        window,
        std::ptr::null_mut(),
        0,
        0,
        0,
        0,
        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

unsafe fn resize_border(window: HWND) -> i32 {
    let dpi = GetDpiForWindow(window);
    GetSystemMetricsForDpi(SM_CYFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
}

unsafe fn over_maximize_button(window: HWND, lparam: LPARAM) -> bool {
    let mut point = POINT {
        x: (lparam & 0xffff) as i16 as i32,
        y: ((lparam >> 16) & 0xffff) as i16 as i32,
    };
    ScreenToClient(window, &mut point);
    let mut client = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    GetClientRect(window, &mut client);
    let scale = GetDpiForWindow(window) as f64 / 96.0;
    let width = (CAPTION_BUTTON_WIDTH * scale).round() as i32;
    let height = (CAPTION_BUTTON_HEIGHT * scale).round() as i32;
    let right = client.right - width;
    point.y >= 0 && point.y < height && point.x >= right - width && point.x < right
}

fn set_hovered(window: HWND, state: &FrameState, hovered: bool) {
    if state.maximize_hovered.swap(hovered, Ordering::Relaxed) != hovered {
        unsafe { InvalidateRect(window, std::ptr::null(), 0) };
    }
}

unsafe extern "system" fn subclass_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    let state = &*(data as *const FrameState);
    match message {
        WM_NCCALCSIZE if wparam != 0 => {
            let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
            let top = params.rgrc[0].top;
            let result = DefSubclassProc(window, message, wparam, lparam);
            if result != 0 {
                return result;
            }
            // Keep the default side and bottom borders but drop the caption.
            // A maximized window hangs its frame off-screen, so its client
            // area must start below that border to stay fully visible.
            params.rgrc[0].top = top + if IsZoomed(window) != 0 {
                resize_border(window)
            } else {
                0
            };
            0
        }
        WM_NCHITTEST => {
            let hit = DefSubclassProc(window, message, wparam, lparam);
            if hit != HTCLIENT as LRESULT {
                return hit;
            }
            if over_maximize_button(window, lparam) {
                return HTMAXBUTTON as LRESULT;
            }
            if IsZoomed(window) == 0 {
                let mut point = POINT {
                    x: (lparam & 0xffff) as i16 as i32,
                    y: ((lparam >> 16) & 0xffff) as i16 as i32,
                };
                ScreenToClient(window, &mut point);
                if point.y < resize_border(window) {
                    return HTTOP as LRESULT;
                }
            }
            hit
        }
        WM_NCMOUSEMOVE => {
            let over = wparam == HTMAXBUTTON as WPARAM;
            if over {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: window,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut track);
            }
            set_hovered(window, state, over);
            DefSubclassProc(window, message, wparam, lparam)
        }
        WM_NCMOUSELEAVE | WM_MOUSEMOVE => {
            set_hovered(window, state, false);
            state.maximize_pressed.store(false, Ordering::Relaxed);
            DefSubclassProc(window, message, wparam, lparam)
        }
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if wparam == HTMAXBUTTON as WPARAM => {
            state.maximize_pressed.store(true, Ordering::Relaxed);
            0
        }
        WM_NCLBUTTONUP if wparam == HTMAXBUTTON as WPARAM => {
            if state.maximize_pressed.swap(false, Ordering::Relaxed) {
                ShowWindow(
                    window,
                    if IsZoomed(window) != 0 {
                        SW_RESTORE
                    } else {
                        SW_MAXIMIZE
                    },
                );
            }
            0
        }
        _ => DefSubclassProc(window, message, wparam, lparam),
    }
}
