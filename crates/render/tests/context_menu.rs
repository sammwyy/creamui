use creamui_core::{Point, Renderer, Size};
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Color, Theme, ThemeProvider};
use creamui_widgets::{ContextMenu, ContextMenuController, ContextMenuItem};

#[test]
fn context_menus_render_with_submenus_in_light_and_dark_themes() {
    for (name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(ThemeProvider::new(theme.clone()));
            let controller = ContextMenuController::new();
            controller.open(
                Point { x: 210.0, y: 70.0 },
                vec![
                    ContextMenuItem::submenu(
                        "new",
                        "New",
                        vec![
                            ContextMenuItem::new("folder", "Folder…", || {}),
                            ContextMenuItem::new("file", "Empty file…", || {}),
                        ],
                    ),
                    ContextMenuItem::new("paste", "Paste", || {}).enabled(false),
                    ContextMenuItem::separator(),
                    ContextMenuItem::new("terminal", "Open terminal here", || {}),
                    ContextMenuItem::new("explorer", "Open in File Explorer", || {}),
                    ContextMenuItem::separator(),
                    ContextMenuItem::new("settings", "Desktop settings…", || {}),
                    ContextMenuItem::new("properties", "Properties", || {}),
                ],
            );
            controller.handle_key(&creamui_core::Key::Down);
            controller.handle_key(&creamui_core::Key::Right);
            let size = Size {
                width: 780.0,
                height: 420.0,
            };
            let mut recorder = SceneRecorder::new();
            recorder.begin(780, 420, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
            let scene = Renderer::new().render(
                Box::new(ContextMenu::new(&controller, size)),
                size,
                &mut recorder,
            );
            let mut raster = Rasterizer::new(780, 420);
            raster.render(&recorder.finish(), &Damage::Full);
            assert_eq!(raster.pixmap().pixel(0, 0).unwrap().alpha(), 0);
            assert!(raster.pixmap().pixel(225, 90).unwrap().alpha() > 0);
            assert!(raster.pixmap().pixel(475, 90).unwrap().alpha() > 0);
            assert!(scene
                .context_menu_at(Point { x: 770.0, y: 410.0 })
                .is_some());
            if let Ok(prefix) = std::env::var("CREAMUI_MENU_SNAPSHOT") {
                raster
                    .pixmap()
                    .save_png(format!("{prefix}-{name}.png"))
                    .unwrap();
            }
        });
    }
}
