//! Render backend selection, chosen by the embedding app at window-creation
//! time and optionally overridden by `CUI_OVERRIDE_RENDER_BACKEND`.

/// Which renderer turns a window's display lists into pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderBackend {
    /// Draw with `wgpu` (the default), falling back to [`RenderBackend::Cpu`]
    /// when no usable adapter exists.
    #[default]
    Gpu,
    /// Rasterize damaged regions on the CPU and present them without any
    /// GPU instance, adapter or device. Useful when no usable GPU driver is
    /// present, or when GPU init cost and memory aren't worth it.
    Cpu,
}

impl RenderBackend {
    #[cfg(not(target_arch = "wasm32"))]
    fn from_env_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gpu" => Some(RenderBackend::Gpu),
            "cpu" => Some(RenderBackend::Cpu),
            _ => None,
        }
    }

    /// Resolves the backend the app requested against
    /// `CUI_OVERRIDE_RENDER_BACKEND` (`gpu` or `cpu`, case-insensitive),
    /// which wins over `requested` if set to a recognized value. An
    /// unrecognized value is ignored (with a warning) rather than treated as
    /// fatal.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn resolve(requested: Self) -> Self {
        let Ok(raw) = std::env::var("CUI_OVERRIDE_RENDER_BACKEND") else {
            return requested;
        };
        match Self::from_env_str(&raw) {
            Some(backend) => {
                if backend != requested {
                    log::info!(
                        "creamui-render: CUI_OVERRIDE_RENDER_BACKEND={raw} overrides requested backend {requested:?} -> {backend:?}"
                    );
                }
                backend
            }
            None => {
                log::warn!(
                    "creamui-render: ignoring CUI_OVERRIDE_RENDER_BACKEND={raw:?}, expected \"gpu\" or \"cpu\""
                );
                requested
            }
        }
    }
}
