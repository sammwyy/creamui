#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("creamui")
            .with_max_level(if cfg!(debug_assertions) {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            })
            .with_filter(
                android_logger::FilterBuilder::new()
                    .parse(if cfg!(debug_assertions) {
                        "info,creamui=debug"
                    } else {
                        "info"
                    })
                    .build(),
            ),
    );
    std::panic::set_hook(Box::new(|panic| log::error!("{panic}")));
    showcase::launch_android(app);
}
