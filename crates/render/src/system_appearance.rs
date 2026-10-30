use super::{AppEvent, EventLoopProxy};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub(super) struct Monitor(Arc<AtomicBool>);
impl Monitor {
    pub(super) fn start(proxy: EventLoopProxy<AppEvent>, follow: bool) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            let stopped = stop.clone();
            if let Err(error) = std::thread::Builder::new()
                .name("creamui-system-theme".into())
                .spawn(move || {
                    let listener = creamui_theme_watch::ThemeListener::start();
                    let mut previous = None;
                    while !stopped.load(Ordering::Acquire) {
                        let appearance = creamui_appearance::query();
                        let resolved = if follow {
                            match creamui_theme_loader::SystemThemeLoader::new()
                                .load_with_system(&appearance)
                            {
                                Ok(resolved) => Some(Box::new(resolved)),
                                Err(error) => {
                                    log::warn!("system theme reload failed: {error}");
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        let current = (appearance, resolved);
                        if previous.as_ref() != Some(&current) {
                            if proxy
                                .send_event(AppEvent::SystemAppearance(
                                    current.0.clone(),
                                    current.1.clone(),
                                ))
                                .is_err()
                            {
                                break;
                            }
                            previous = Some(current);
                        }
                        while !stopped.load(Ordering::Acquire)
                            && !listener.recv_timeout(std::time::Duration::from_millis(200))
                        {
                        }
                    }
                })
            {
                log::error!("failed to start system theme monitor: {error}");
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let _ = (proxy, follow);
        Self(stop)
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
