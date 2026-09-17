//! Verifies the `perf-metrics` counters wired into `creamui-core` and
//! `creamui-render` reflect the engine's actual work, and pins that
//! behavior so a later change to it shows up as a failing assertion here.

use creamui_bench::metrics::measure;
use creamui_bench::painter::{FramePipeline, NoopPainter};
use creamui_bench::scenes;
use creamui_core::{Point, Renderer, Size};
use creamui_render::Damage;
use creamui_theme::Color;

const VIEWPORT: Size = Size {
    width: 800.0,
    height: 600.0,
};

#[test]
fn initial_mount_visits_and_writes_every_node_once() {
    let mut renderer = Renderer::new();
    let mut painter = NoopPainter;
    let (_, metrics) = measure(|| {
        renderer.render(scenes::wide_tree(100), VIEWPORT, &mut painter);
    });
    let node_count = 101;
    assert_eq!(metrics.root_builds, 1);
    assert_eq!(metrics.layout_runs, 1);
    assert_eq!(metrics.reconcile_visits, node_count);
    assert_eq!(metrics.widget_objects_built, node_count);
    assert_eq!(metrics.taffy_style_writes, node_count);
    assert_eq!(metrics.taffy_children_writes, node_count);
    assert_eq!(metrics.taffy_context_writes, node_count);
    // One flow pass and one absolute pass over every node.
    assert_eq!(metrics.paint_nodes_visited, node_count * 2);
    assert_eq!(metrics.paint_nodes_recorded, node_count);
}

#[test]
fn unchanged_rerender_performs_zero_taffy_writes() {
    let mut renderer = Renderer::new();
    let mut painter = NoopPainter;
    renderer.render(scenes::wide_tree(100), VIEWPORT, &mut painter);

    let (_, metrics) = measure(|| {
        renderer.render(scenes::wide_tree(100), VIEWPORT, &mut painter);
    });

    assert_eq!(metrics.taffy_style_writes, 0);
    assert_eq!(metrics.taffy_context_writes, 0);
    assert_eq!(metrics.taffy_children_writes, 0);
    assert_eq!(metrics.reconcile_visits, 101);
}

#[test]
fn single_leaf_paint_change_touches_no_taffy_state() {
    let mut renderer = Renderer::new();
    let mut painter = NoopPainter;
    renderer.render(scenes::wide_tree(500), VIEWPORT, &mut painter);

    let (_, metrics) = measure(|| {
        renderer.render(
            scenes::wide_tree_with_leaf_color(500, 0, Color::rgb(255, 0, 0)),
            VIEWPORT,
            &mut painter,
        );
    });

    let node_count = 501;
    assert_eq!(metrics.reconcile_visits, node_count);
    assert_eq!(metrics.paint_nodes_visited, node_count * 2);
    assert_eq!(metrics.taffy_style_writes, 0);
    assert_eq!(metrics.taffy_context_writes, 0);
    assert_eq!(metrics.taffy_children_writes, 0);
}

#[test]
fn a_single_leaf_change_rasterizes_only_that_leaf() {
    let mut renderer = Renderer::new();
    let mut frames = FramePipeline::new(800, 600, 1.0);
    renderer.update(scenes::wide_tree(500), VIEWPORT);
    let (first, metrics) = measure(|| frames.frame(&renderer));
    assert_eq!(first.damage, Damage::Full);
    assert_eq!(first.items, 500);
    assert_eq!(metrics.display_items, 500);
    assert_eq!(metrics.cpu_pixels_rasterized, 800 * 600);

    renderer.update(
        scenes::wide_tree_with_leaf_color(500, 7, Color::rgb(255, 0, 0)),
        VIEWPORT,
    );
    let (second, metrics) = measure(|| frames.frame(&renderer));
    assert!(matches!(second.damage, Damage::Partial(ref r) if r.len() == 1));
    assert!(second.damaged_pixels <= 18.0 * 18.0);
    assert!(metrics.cpu_pixels_rasterized <= 18 * 18);
}

#[test]
fn an_unchanged_frame_rasterizes_nothing() {
    let mut renderer = Renderer::new();
    let mut frames = FramePipeline::new(800, 600, 1.0);
    renderer.update(scenes::dashboard(), VIEWPORT);
    frames.frame(&renderer);
    let (output, metrics) = measure(|| frames.frame(&renderer));
    assert_eq!(output.damage, Damage::None);
    assert_eq!(metrics.cpu_pixels_rasterized, 0);
    assert_eq!(metrics.text_layouts, 0, "unchanged text reuses its layout");
}

#[test]
fn hovering_repaints_only_the_hover_target() {
    let mut renderer = Renderer::new();
    let mut frames = FramePipeline::new(800, 600, 1.0);
    renderer.update(scenes::hover_list(50), VIEWPORT);
    frames.frame(&renderer);
    frames.set_pointer(Some(Point { x: 10.0, y: 5.0 }));
    let output = frames.frame(&renderer);
    assert!(matches!(output.damage, Damage::Partial(_)));
    assert!(output.damaged_pixels < 800.0 * 30.0);
}
