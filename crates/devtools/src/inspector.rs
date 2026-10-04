use crate::DEBUG_FONT_FAMILY;
use creamui_core::runtime::{RuntimeInspection, RuntimeNodeId};
use creamui_core::{Painter, Rect, Size, TextAlign};
use creamui_render::DevtoolsCommand;
use creamui_theme::Color;
use std::fmt::Write;

#[derive(Default)]
pub(crate) struct Inspector {
    tree: bool,
    damage: bool,
    layout: bool,
    hits: bool,
    selected: Option<RuntimeNodeId>,
    snapshot: RuntimeInspection,
    damage_regions: Vec<Rect>,
}

impl Inspector {
    pub(crate) fn enabled(&self) -> bool {
        self.tree || self.damage || self.layout || self.hits
    }

    pub(crate) fn command(&mut self, command: DevtoolsCommand) -> bool {
        match command {
            DevtoolsCommand::ToggleTree => self.tree = !self.tree,
            DevtoolsCommand::ToggleDamage => self.damage = !self.damage,
            DevtoolsCommand::ToggleLayout => self.layout = !self.layout,
            DevtoolsCommand::ToggleHitRegions => self.hits = !self.hits,
            DevtoolsCommand::NextNode | DevtoolsCommand::PreviousNode => {
                if !self.tree || self.snapshot.nodes.is_empty() {
                    return false;
                }
                let count = self.snapshot.nodes.len();
                let current = self.selected_index();
                let next = if command == DevtoolsCommand::NextNode {
                    (current + 1) % count
                } else {
                    (current + count - 1) % count
                };
                self.selected = Some(self.snapshot.nodes[next].id);
            }
        }
        if !self.enabled() {
            self.snapshot = RuntimeInspection::default();
            self.damage_regions.clear();
            self.selected = None;
        }
        true
    }

    fn selected_index(&self) -> usize {
        self.selected
            .and_then(|id| self.snapshot.nodes.iter().position(|node| node.id == id))
            .unwrap_or(0)
    }

    pub(crate) fn inspect(&mut self, mut snapshot: RuntimeInspection, damage: &[Rect]) {
        if !snapshot
            .nodes
            .iter()
            .any(|node| !node.invalidations.is_empty())
        {
            let previous: std::collections::HashMap<_, _> = self
                .snapshot
                .nodes
                .iter()
                .filter(|node| !node.invalidations.is_empty())
                .map(|node| (node.id, node.invalidations))
                .collect();
            for node in &mut snapshot.nodes {
                if let Some(&flags) = previous.get(&node.id) {
                    node.invalidations = flags;
                }
            }
        }
        if damage
            .iter()
            .any(|rect| rect.width > 0.0 && rect.height > 0.0)
        {
            self.damage_regions.clear();
            for &rect in damage
                .iter()
                .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
            {
                if self.damage_regions.len() < 64 {
                    self.damage_regions.push(rect);
                } else {
                    let bounds = self
                        .damage_regions
                        .iter()
                        .copied()
                        .fold(rect, |bounds, rect| bounds.union(rect));
                    self.damage_regions.clear();
                    self.damage_regions.push(bounds);
                }
            }
        }
        self.snapshot = snapshot;
        self.selected = self
            .snapshot
            .nodes
            .get(self.selected_index())
            .map(|node| node.id);
    }

    pub(crate) fn paint(&self, painter: &mut dyn Painter, viewport: Size) {
        if !self.enabled() {
            return;
        }
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: viewport.width,
            height: viewport.height,
        };
        painter.push_clip(bounds);
        if self.damage {
            for &rect in &self.damage_regions {
                if let Some(rect) = rect.intersect(bounds) {
                    painter.fill_rect(rect, Color::rgba(255, 70, 60, 35), 0.0);
                    painter.stroke_rect_inside(rect, Color::rgb(255, 70, 60), 1.0, 0.0);
                }
            }
        }
        for node in &self.snapshot.nodes {
            if self.layout {
                if let Some(rect) = node.visual_rect.intersect(bounds) {
                    painter.stroke_rect_inside(rect, Color::rgba(80, 160, 255, 180), 1.0, 0.0);
                }
            }
            if self.hits {
                if let Some(rect) = node.hit_rect.and_then(|rect| rect.intersect(bounds)) {
                    painter.fill_rect(rect, Color::rgba(60, 230, 140, 20), 0.0);
                    painter.stroke_rect_inside(rect, Color::rgb(60, 230, 140), 1.0, 0.0);
                }
            }
        }
        if self.tree {
            if let Some(node) = self.snapshot.nodes.get(self.selected_index()) {
                if let Some(rect) = node.visual_rect.intersect(bounds) {
                    painter.stroke_rect_inside(rect, Color::rgb(255, 220, 80), 2.0, 0.0);
                }
            }
            self.paint_tree(painter, viewport);
        }
        painter.pop_clip();
    }

    fn paint_tree(&self, painter: &mut dyn Painter, viewport: Size) {
        let panel = Rect {
            x: 8.0,
            y: 8.0,
            width: (viewport.width - 16.0).clamp(0.0, 420.0),
            height: (viewport.height - 16.0).clamp(0.0, 520.0),
        };
        if panel.width <= 0.0 || panel.height <= 0.0 {
            return;
        }
        painter.fill_rect(panel, Color::rgba(15, 20, 30, 235), 6.0);
        painter.push_clip(panel);
        let text = self.tree_text(((panel.height - 145.0).max(0.0) / 15.0) as usize);
        painter.fill_text_font(
            Rect {
                x: panel.x + 10.0,
                y: panel.y + 10.0,
                width: (panel.width - 20.0).max(0.0),
                height: (panel.height - 20.0).max(0.0),
            },
            &text,
            Color::rgb(220, 230, 240),
            12.0,
            TextAlign::Start,
            Some(DEBUG_FONT_FAMILY),
            false,
            false,
        );
        painter.pop_clip();
    }

    fn tree_text(&self, rows: usize) -> String {
        let mut text = format!("Runtime tree: {} nodes, layout {}\nF4 tree  F5 damage  F6 layout  F7 hits\nF8 next  Shift+F8 previous\n", self.snapshot.nodes.len(), self.snapshot.layout_epoch);
        let selected = self.selected_index();
        let start = selected
            .saturating_sub(rows / 2)
            .min(self.snapshot.nodes.len().saturating_sub(rows));
        for (index, node) in self
            .snapshot
            .nodes
            .iter()
            .enumerate()
            .skip(start)
            .take(rows)
        {
            let _ = writeln!(
                text,
                "{}{}{} {:x}",
                if index == selected { "> " } else { "  " },
                "  ".repeat(node.depth.min(8)),
                node.kind,
                node.id.to_bits()
            );
        }
        if let Some(node) = self.snapshot.nodes.get(selected) {
            let rect = node.layout_rect;
            let _ = writeln!(
                text,
                "\n{:x}  parent {}",
                node.id.to_bits(),
                node.parent
                    .map_or_else(|| "none".into(), |id| format!("{:x}", id.to_bits()))
            );
            let _ = writeln!(
                text,
                "layout {:.1},{:.1}  {:.1} x {:.1}",
                rect.x, rect.y, rect.width, rect.height
            );
            let _ = writeln!(
                text,
                "opacity {:.2}  focus {}  hit {}",
                node.opacity,
                node.focusable,
                node.hit_rect.is_some()
            );
            let _ = writeln!(text, "invalidated: {:?}", node.invalidations);
            if let Some(label) = &node.text {
                let label: String = label
                    .chars()
                    .take(60)
                    .map(|ch| if ch.is_control() { ' ' } else { ch })
                    .collect();
                let _ = writeln!(text, "text: {label}");
            }
        } else {
            text.push_str("\nNo retained runtime nodes");
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::runtime::{DirtyFlags, NodeKind, Runtime};

    fn snapshot() -> RuntimeInspection {
        let mut runtime = Runtime::new();
        runtime.set_inspection_enabled(true);
        let root = runtime.transaction().create_node(NodeKind::Container);
        let child = runtime.transaction().create_node(NodeKind::Container);
        runtime.transaction().insert_child(root, child, None);
        runtime.set_root(Some(root));
        runtime.take_inspection().unwrap()
    }

    #[test]
    fn region_overlays_rasterize_only_their_visible_geometry() {
        use creamui_render::{Damage, Rasterizer, SceneRecorder};
        use creamui_theme::ColorScheme;

        let mut inspector = Inspector::default();
        let mut snapshot = snapshot();
        snapshot.nodes.truncate(1);
        snapshot.nodes[0].visual_rect = Rect {
            x: 10.0,
            y: 10.0,
            width: 30.0,
            height: 30.0,
        };
        snapshot.nodes[0].hit_rect = Some(Rect {
            x: 15.0,
            y: 15.0,
            width: 10.0,
            height: 10.0,
        });
        let damage = Rect {
            x: 50.0,
            y: 50.0,
            width: 10.0,
            height: 10.0,
        };
        inspector.inspect(snapshot, &[damage]);
        inspector.command(DevtoolsCommand::ToggleDamage);
        inspector.command(DevtoolsCommand::ToggleLayout);
        inspector.command(DevtoolsCommand::ToggleHitRegions);
        let mut recorder = SceneRecorder::new();
        recorder.begin(80, 80, 1.0, Color::rgb(0, 0, 0), ColorScheme::light());
        inspector.paint(
            &mut recorder,
            Size {
                width: 80.0,
                height: 80.0,
            },
        );
        let list = recorder.finish();
        let mut raster = Rasterizer::new(80, 80);
        raster.render(&list, &Damage::Full);
        let layout = raster.pixmap().pixel(10, 20).unwrap();
        assert!(layout.blue() > layout.red());
        let hit = raster.pixmap().pixel(15, 20).unwrap();
        assert!(hit.green() > hit.red());
        let damaged = raster.pixmap().pixel(50, 55).unwrap();
        assert!(damaged.red() > damaged.green());
        let outside = raster.pixmap().pixel(75, 75).unwrap();
        assert_eq!([outside.red(), outside.green(), outside.blue()], [0, 0, 0]);
    }

    #[test]
    fn selection_follows_identity_across_reorders_and_handles_removal() {
        let mut inspector = Inspector::default();
        inspector.command(DevtoolsCommand::ToggleTree);
        let mut snapshot = snapshot();
        inspector.inspect(snapshot.clone(), &[]);
        inspector.command(DevtoolsCommand::NextNode);
        let child = snapshot.nodes[1].id;
        assert_eq!(inspector.selected, Some(child));
        snapshot.nodes.reverse();
        inspector.inspect(snapshot.clone(), &[]);
        assert_eq!(inspector.selected, Some(child));
        snapshot.nodes.remove(0);
        inspector.inspect(snapshot, &[]);
        assert_ne!(inspector.selected, Some(child));
        inspector.command(DevtoolsCommand::ToggleTree);
        assert!(!inspector.enabled());
        assert!(inspector.snapshot.nodes.is_empty());
    }

    #[test]
    fn idle_refresh_preserves_the_last_invalidation_and_damage() {
        let mut inspector = Inspector::default();
        let mut snapshot = snapshot();
        let rect = Rect {
            x: 1.0,
            y: 2.0,
            width: 10.0,
            height: 10.0,
        };
        inspector.inspect(snapshot.clone(), &[rect]);
        for node in &mut snapshot.nodes {
            node.invalidations = DirtyFlags::empty();
        }
        inspector.inspect(snapshot, &[]);
        assert!(inspector.snapshot.nodes[0]
            .invalidations
            .contains(DirtyFlags::STRUCTURE));
        assert_eq!(inspector.damage_regions, [rect]);
        assert!(inspector.tree_text(2).contains("STRUCTURE"));
    }

    #[test]
    fn large_damage_lists_are_bounded_and_cover_all_input_regions() {
        let mut inspector = Inspector::default();
        let damage: Vec<_> = (0..1000)
            .map(|i| Rect {
                x: i as f32,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            })
            .collect();
        inspector.inspect(snapshot(), &damage);
        assert!(inspector.damage_regions.len() <= 64);
        for rect in damage {
            assert!(inspector
                .damage_regions
                .iter()
                .any(|region| region.intersect(rect) == Some(rect)));
        }
    }
}
