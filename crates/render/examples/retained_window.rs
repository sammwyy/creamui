use creamui_core::runtime::{EventState, MountCx, Mutation, NodeKind, Runtime, SharedRuntime};
use creamui_core::{PaintStyle, Style, TypographyStyle};
use creamui_reactive::{Owner, Signal};
use creamui_render::{AppBuilder, RenderBackend, WindowOptions};
use creamui_theme::Color;
use std::rc::Rc;

fn main() {
    let mut tree = Runtime::new();
    let root = tree.transaction().create_node(NodeKind::Container);
    tree.set_root(Some(root));
    let runtime = SharedRuntime::new(tree);
    let owner = Owner::new();
    let cx = MountCx::new(runtime.clone(), owner.clone(), root);
    let label = cx.text("Click anywhere to change the color");
    runtime.transaction(|tx| {
        tx.apply(Mutation::SetLayoutStyle {
            node: root,
            style: Style::new().width(360.0).height(180.0).layout,
        });
        tx.apply(Mutation::SetLayoutStyle {
            node: label,
            style: Style::new().width(360.0).height(48.0).layout,
        });
        tx.apply(Mutation::SetTypographyStyle {
            node: label,
            style: TypographyStyle {
                color: Some(Color::rgb(255, 255, 255).into()),
                font_size: Some(18.0),
                ..Default::default()
            },
        });
    });

    let selected = Signal::new(false);
    cx.bind({
        let selected = selected.clone();
        move |tx| {
            tx.apply(Mutation::SetPaintStyle {
                node: root,
                style: PaintStyle {
                    background: Some(if selected.get() {
                        Color::rgb(38, 108, 167).into()
                    } else {
                        Color::rgb(123, 60, 135).into()
                    }),
                    ..Default::default()
                },
            });
        }
    });
    cx.set_events(
        root,
        EventState {
            on_click: Some(Rc::new(move || selected.set(!selected.get()))),
            ..Default::default()
        },
    );

    AppBuilder::new()
        .runtime_window(
            WindowOptions {
                title: "Retained runtime".into(),
                width: 360,
                height: 180,
                backend: RenderBackend::Cpu,
                ..WindowOptions::default()
            },
            Color::rgb(0, 0, 0),
            |_| {},
            runtime,
        )
        .run();
}
