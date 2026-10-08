fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    creamui_router_example::app().run();
}
