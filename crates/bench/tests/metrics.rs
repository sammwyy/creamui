//! Verifies the `perf-metrics` counters wired into `creamui-core` correctly
//! reflect the engine's actual reconcile/layout/paint behavior, and pins
//! that behavior so a later change to it shows up as a failing assertion
//! here.

use creamui_bench::metrics::measure;
use creamui_bench::painter::{raster_painter, NoopPainter};
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
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
    let node_count = 101; // root + 100 leaves
    assert_eq!(metrics.root_builds, 1);
    assert_eq!(metrics.layout_runs, 1);
    assert_eq!(metrics.reconcile_visits, node_count);
    assert_eq!(metrics.widget_objects_built, node_count);
    assert_eq!(metrics.taffy_style_writes, node_count);
    assert_eq!(metrics.taffy_children_writes, node_count);
    assert_eq!(metrics.taffy_context_writes, node_count);
    // Every node is walked once per paint pass (flow, then absolute) even
    // though nothing in this tree is absolutely positioned — see
    // `scene::paint_instance`'s two-pass design.
    assert_eq!(metrics.paint_nodes_visited, node_count * 2);
    assert_eq!(metrics.paint_nodes_recorded, node_count);
}

#[test]
fn unchanged_rerender_performs_zero_taffy_writes() {
    // `scene::reconcile` diffs against the previous frame's `taffy::Style`,
    // measure fingerprint, and child-id list before writing any of them.
    let mut renderer = Renderer::new();
    let mut painter = NoopPainter;
    renderer.render(scenes::wide_tree(100), VIEWPORT, &mut painter);

    let (_, metrics) = measure(|| {
        renderer.render(scenes::wide_tree(100), VIEWPORT, &mut painter);
    });

    assert_eq!(metrics.taffy_style_writes, 0);
    assert_eq!(metrics.taffy_context_writes, 0);
    assert_eq!(metrics.taffy_children_writes, 0);
    // Reconciliation still walks the whole ephemeral `Widget` tree to
    // discover that nothing changed — that's the work Phase 2+ removes.
    assert_eq!(metrics.reconcile_visits, 101);
}

#[test]
fn single_leaf_paint_change_touches_no_taffy_state() {
    // A background-color change affects only paint, not layout, so it must
    // not write to `taffy` at all, even though the `Widget` tree is still
    // rebuilt and reconciled from the root.
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
fn animated_leaves_are_pruned_from_an_animation_only_repaint() {
    let mut renderer = Renderer::new();
    let mut painter = NoopPainter;
    // Two full renders so the animated leaves' streak crosses the
    // layer-promotion threshold.
    renderer.render(scenes::animated_cards(2_000, 3), VIEWPORT, &mut painter);
    renderer.render(scenes::animated_cards(2_000, 3), VIEWPORT, &mut painter);

    let (_, metrics) = measure(|| {
        renderer.repaint_animated(&mut painter, None, false);
    });

    assert!(
        metrics.paint_nodes_visited < 100,
        "an animation-only repaint should prune the 2,000-node static \
         subtree instead of walking it; visited {}",
        metrics.paint_nodes_visited
    );
}

#[test]
fn raster_painter_reports_real_cpu_pixel_work() {
    let mut renderer = Renderer::new();
    let mut painter = raster_painter(800, 600);
    let (_, metrics) = measure(|| {
        renderer.render(scenes::wide_tree(200), VIEWPORT, &mut painter);
    });
    assert!(
        metrics.cpu_pixels_rasterized > 0,
        "expected the tiny-skia backend to report rasterized pixel area"
    );
}
