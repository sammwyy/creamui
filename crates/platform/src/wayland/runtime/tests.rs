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
                    window.set_min_inner_size(Some(LogicalSize::new(640.0, 480.0)));
                    window.set_minimized(false);
                    window.set_maximized(false);
                    window.set_maximized(true);
                }
                2 => {
                    assert!(decorations.maximized);
                    window.set_min_inner_size(Some(LogicalSize::new(960.0, 560.0)));
                    window.set_maximized(false);
                }
                3 => {
                    assert!(!decorations.maximized);
                    window.set_min_inner_size(None);
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
    assert_eq!(server.minimums(), [(1, 1), (640, 480), (960, 560), (0, 0)]);
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

#[test]
fn nested_popups_preserve_anchors_grabs_and_destroy_before_sibling_replacement() {
    struct PopupApp {
        windows: Vec<Arc<dyn PlatformWindow>>,
        phase: usize,
        deadline: Instant,
        closed: bool,
    }
    impl ApplicationHandler<Event> for PopupApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop<'_>) {
            self.windows.push(
                event_loop
                    .create_window(WindowAttributes::default())
                    .unwrap(),
            );
        }
        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop<'_>,
            id: WindowId,
            event: WindowEvent,
        ) {
            if !matches!(event, WindowEvent::Resized(_)) {
                return;
            }
            match self.phase {
                0 => {
                    assert_eq!(id, self.windows[0].id());
                    assert!(self.windows[0].supports_native_popups());
                    self.windows.push(
                        event_loop
                            .create_popup(
                                WindowAttributes {
                                    size: LogicalSize::new(248.0, 900.0),
                                    decorations: false,
                                    ..Default::default()
                                },
                                PopupOptions {
                                    parent: id,
                                    anchor_x: 635.0,
                                    anchor_y: 395.0,
                                    anchor_width: 1.0,
                                    anchor_height: 1.0,
                                    input_serial: Some(crate::InputSerial(42)),
                                    placement: crate::PopupPlacement::RightTop,
                                    resize_to_fit: true,
                                },
                            )
                            .unwrap(),
                    );
                }
                1 | 2 => {
                    let root = self.windows[1].clone();
                    assert_eq!(
                        root.inner_size(),
                        PhysicalSize {
                            width: 248,
                            height: 180
                        }
                    );
                    if self.phase == 2 {
                        self.windows[2].close();
                    }
                    self.windows.push(
                        event_loop
                            .create_popup(
                                WindowAttributes {
                                    size: LogicalSize::new(248.0, 100.0),
                                    decorations: false,
                                    ..Default::default()
                                },
                                PopupOptions {
                                    parent: root.id(),
                                    anchor_x: 5.0,
                                    anchor_y: 5.0,
                                    anchor_width: 238.0,
                                    anchor_height: 32.0,
                                    input_serial: Some(crate::InputSerial(42)),
                                    placement: crate::PopupPlacement::RightTop,
                                    resize_to_fit: true,
                                },
                            )
                            .unwrap(),
                    );
                }
                3 => {
                    self.windows[3].close();
                    self.windows[1].close();
                    self.windows[0].close();
                }
                _ => return,
            }
            self.phase += 1;
        }
        fn user_event(&mut self, event_loop: &ActiveEventLoop<'_>, event: Event) {
            if matches!(event, Event::Closed) {
                self.closed = true;
                event_loop.exit();
            }
        }
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop<'_>) {
            if Instant::now() >= self.deadline {
                event_loop.exit();
            }
        }
    }
    let event_loop = EventLoop::<Event>::with_user_event().build().unwrap();
    let (server, connection) = compositor::Compositor::start(event_loop.create_proxy());
    let mut app = PopupApp {
        windows: Vec::new(),
        phase: 0,
        deadline: Instant::now() + Duration::from_secs(5),
        closed: false,
    };
    event_loop
        .run_app_on_connection(&mut app, connection)
        .unwrap();
    assert!(app.closed);
    assert_eq!(app.phase, 4);
    let popups = server.popups();
    assert_eq!(popups.len(), 3);
    assert_eq!(popups[0].positioner.anchor, (635, 395, 1, 1));
    assert_eq!(popups[1].parent, popups[0].surface);
    assert_eq!(popups[2].parent, popups[0].surface);
    assert_ne!(popups[0].parent, popups[0].surface);
    assert!(popups.iter().all(|popup| popup.serial == Some(42)));
    let resize =
        wayland_protocols::xdg::shell::client::xdg_positioner::ConstraintAdjustment::ResizeY.bits();
    assert!(popups
        .iter()
        .all(|popup| popup.positioner.adjustments & resize != 0));
    assert_eq!(
        server.popup_closes(),
        vec![popups[1].popup, popups[2].popup, popups[0].popup]
    );
}
