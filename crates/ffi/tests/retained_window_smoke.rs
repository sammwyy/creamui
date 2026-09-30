use creamui::runtime::{
    cui_create_node, cui_run_window, cui_runtime_free, cui_runtime_new, cui_runtime_node_count,
    cui_set_background, cui_set_root, CRuntime,
};
use creamui::{creamui_window_close, creamui_window_handle_free, CColor, CWindowHandle};
use creamui_abi::{
    CBlurRegion, CWindowOptions, CWindowOptionsV2, CUI_BLUR_NONE, CUI_RENDER_BACKEND_CPU,
};
use std::ffi::{c_void, CString};

extern "C" fn ready(handle: *mut CWindowHandle, userdata: *mut c_void) {
    let rt = userdata.cast::<CRuntime>();
    unsafe {
        assert_eq!(cui_runtime_node_count(rt), 1);
        creamui_window_close(handle);
        creamui_window_handle_free(handle);
    }
}

fn main() {
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return;
    }

    let title = CString::new("retained C window").unwrap();
    let rt = cui_runtime_new();
    unsafe {
        let root = cui_create_node(rt, 0);
        cui_set_root(rt, root);
        cui_set_background(rt, root, CColor::rgb(20, 40, 60), 0.0);
        cui_run_window(
            rt,
            CWindowOptionsV2 {
                base: CWindowOptions {
                    title: title.as_ptr(),
                    width: 64,
                    height: 64,
                    resizable: 0,
                    decorations: 0,
                    transparent: 0,
                    backend: CUI_RENDER_BACKEND_CPU,
                },
                blur: CBlurRegion {
                    kind: CUI_BLUR_NONE,
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
            },
            CColor::rgb(0, 0, 0),
            Some(ready),
            rt.cast(),
        );
        cui_runtime_free(rt);
    }
}
