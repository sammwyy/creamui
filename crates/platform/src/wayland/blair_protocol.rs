#![allow(
    dead_code,
    missing_docs,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_imports,
    unused_unsafe,
    unused_variables,
    clippy::all
)]

#[cfg(feature = "blur-blair")]
pub(crate) mod blur {
    pub(crate) mod client {
        use wayland_client;
        use wayland_client::protocol::*;

        pub mod __interfaces {
            use wayland_client::protocol::__interfaces::*;

            wayland_scanner::generate_interfaces!("protocol/blair-blur-v1.xml");
        }

        use self::__interfaces::*;

        wayland_scanner::generate_client_code!("protocol/blair-blur-v1.xml");
    }
}

#[cfg(feature = "window-integration-blair")]
pub(crate) mod window_integration {
    pub(crate) mod client {
        use wayland_client;
        use wayland_client::protocol::*;

        pub mod __interfaces {
            use wayland_client::protocol::__interfaces::*;

            wayland_scanner::generate_interfaces!("protocol/blair-window-integration-v1.xml");
        }

        use self::__interfaces::*;

        wayland_scanner::generate_client_code!("protocol/blair-window-integration-v1.xml");
    }
}
