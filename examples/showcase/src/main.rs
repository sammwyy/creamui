//! Native entry point — the actual app lives in the `showcase` library
//! crate (`src/lib.rs`) so `demo/showcase` (the WASM build) can reuse it.
//! Press F3 for the devtools overlay; set `CUI_FRAME_LOG=1` to print frame
//! timings.

fn main() {
    creamui_devtools::init();
    showcase::launch();
}
