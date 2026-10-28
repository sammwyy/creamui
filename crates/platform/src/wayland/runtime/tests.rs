mod compositor;

use super::*;

#[derive(Clone, Copy)]
enum Event {
    Minimized,
    Closed,
}

struct App {
    decorations: bool,
    window: Option<Arc<dyn PlatformWindow>>,
    states: Vec<WindowDecorations>,
    deadline: Instant,
    closed: bool,
}

impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop<'_>) {
        let window = event_loop
            .create_window(WindowAttributes {
                decorations: self.decorations,
                ..Default::default()
            })
            .unwrap();
        window.set_compositor_integration(Some(CompositorIntegrationRequest::Titlebar));
        self.window = Some(window);
    }

    fn window_event(&mut self, _: &ActiveEventLoop<'_>, id: WindowId, event: WindowEvent) {
        let window = self.window.as_ref().unwrap();
        assert_eq!(id, window.id());
        if let WindowEvent::DecorationsChanged(decorations) = event {
            assert_eq!(decorations, window.window_decorations());
            assert_eq!(decorations.mode, crate::WindowDecorationMode::Client);
            self.states.push(decorations);
            match self.states.len() {
                1 => {
                    assert!(!decorations.maximized);
                    window.set_minimized(false);
                    window.set_maximized(false);
                    window.set_maximized(true);
                }
                2 => {
                    assert!(decorations.maximized);
                    window.set_maximized(false);
                }
                3 => {
                    assert!(!decorations.maximized);
                    window.set_minimized(true);
                }
                _ => panic!("unexpected configure"),
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop<'_>, event: Event) {
        match event {
            Event::Minimized => {
                let window = self.window.as_ref().unwrap();
                window.close();
                window.set_maximized(true);
                window.set_minimized(true);
            }
            Event::Closed => {
                self.closed = true;
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop<'_>) {
        if Instant::now() >= self.deadline {
            event_loop.exit();
        }
    }
}

#[test]
fn titlebar_actions_reach_xdg_shell_and_configures_report_maximized_state() {
    for decorations in [true, false] {
        exercise_titlebar(decorations);
    }
}

fn exercise_titlebar(decorations: bool) {
    let event_loop = EventLoop::<Event>::with_user_event().build().unwrap();
    let (server, connection) = compositor::Compositor::start(event_loop.create_proxy());
    let mut app = App {
        decorations,
        window: None,
        states: Vec::new(),
        deadline: Instant::now() + Duration::from_secs(5),
        closed: false,
    };
    event_loop
        .run_app_on_connection(&mut app, connection)
        .unwrap();
    assert!(
        app.closed,
        "window actions did not finish before the deadline"
    );
    assert_eq!(
        app.states
            .iter()
            .map(|state| state.maximized)
            .collect::<Vec<_>>(),
        [false, true, false]
    );
    assert_eq!(
        server.actions(),
        ["maximize", "restore", "minimize", "close"]
    );
}

#[test]
fn panel_resize_preserves_anchored_length_and_updates_the_exclusive_zone() {
    for role in [WindowRole::TopPanel, WindowRole::BottomPanel] {
        assert_eq!(
            panel_dimensions(role, LogicalSize::new(800.0, 2.0)),
            (0, 2, 2)
        );
        assert_eq!(
            panel_dimensions(role, LogicalSize::new(800.0, 76.0)),
            (0, 76, 76)
        );
    }
    for role in [WindowRole::LeftPanel, WindowRole::RightPanel] {
        assert_eq!(
            panel_dimensions(role, LogicalSize::new(2.0, 800.0)),
            (2, 0, 2)
        );
        assert_eq!(
            panel_dimensions(role, LogicalSize::new(76.0, 800.0)),
            (76, 0, 76)
        );
    }
}

#[test]
fn terminal_function_keys_and_insert_are_preserved() {
    for (code, expected) in [
        (29, Key::Control),
        (97, Key::Control),
        (54, Key::Shift),
        (100, Key::Alt),
        (125, Key::Super),
        (126, Key::Super),
        (15, Key::Tab),
        (114, Key::Media("XF86AudioLowerVolume".into())),
        (115, Key::Media("XF86AudioRaiseVolume".into())),
        (224, Key::Media("XF86MonBrightnessDown".into())),
        (225, Key::Media("XF86MonBrightnessUp".into())),
        (248, Key::Media("XF86AudioMicMute".into())),
        (110, Key::Insert),
        (59, Key::F1),
        (60, Key::F2),
        (67, Key::F9),
        (68, Key::F10),
        (87, Key::F11),
        (88, Key::F12),
    ] {
        assert_eq!(keyboard_key(code, None), expected);
    }
}

#[test]
fn resizable_windows_do_not_use_their_initial_size_as_a_minimum() {
    let size = LogicalSize::new(1120.0, 760.0);
    assert_eq!(window_size_limits(size, true), ((1, 1), None));
    assert_eq!(
        window_size_limits(size, false),
        ((1120, 760), Some((1120, 760)))
    );
}

#[test]
fn all_resize_directions_map_to_wayland_edges() {
    for (direction, edge) in [
        (ResizeDirection::North, ResizeEdge::Top),
        (ResizeDirection::South, ResizeEdge::Bottom),
        (ResizeDirection::East, ResizeEdge::Right),
        (ResizeDirection::West, ResizeEdge::Left),
        (ResizeDirection::NorthWest, ResizeEdge::TopLeft),
        (ResizeDirection::NorthEast, ResizeEdge::TopRight),
        (ResizeDirection::SouthWest, ResizeEdge::BottomLeft),
        (ResizeDirection::SouthEast, ResizeEdge::BottomRight),
    ] {
        assert_eq!(resize_edge(direction), edge);
    }
}

#[test]
fn wayland_axis_deltas_match_the_platform_scroll_convention() {
    assert_eq!(
        wayland_axis_delta(wl_pointer::Axis::VerticalScroll, 12.0),
        Some(MouseScrollDelta::PixelDelta(PhysicalPosition {
            x: 0.0,
            y: -12.0
        }))
    );
    assert_eq!(
        wayland_axis_delta(wl_pointer::Axis::HorizontalScroll, 8.0),
        Some(MouseScrollDelta::PixelDelta(PhysicalPosition {
            x: -8.0,
            y: 0.0
        }))
    );
}

#[test]
fn refresh_interval_uses_the_current_positive_mode() {
    let mode = |refresh_rate, current| Mode {
        dimensions: (1920, 1080),
        refresh_rate,
        current,
        preferred: false,
    };
    assert_eq!(
        current_refresh_interval(&[mode(144_000, false), mode(120_000, true)]),
        Some(Duration::from_nanos(8_333_333))
    );
    assert_eq!(current_refresh_interval(&[mode(0, true)]), None);
    assert_eq!(current_refresh_interval(&[mode(60_000, false)]), None);
}
