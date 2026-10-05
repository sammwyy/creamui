use super::arena::{Arena, Id};
use super::RuntimeNodeId;
use std::collections::{HashMap, HashSet};
use taffy::{
    geometry::{Point, Size},
    style::{AvailableSpace, Display, Style},
    Cache, CacheTree, Layout, LayoutInput, LayoutOutput, LayoutPartialTree, NodeId, RunMode,
    TaffyError, TraversePartialTree,
};

type Result<T> = std::result::Result<T, TaffyError>;

struct LayoutNode {
    style: Style,
    measure: Option<crate::MeasureFn>,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    cache: Cache,
    unrounded: Layout,
    rounded: Layout,
    cumulative: Point<f32>,
    runtime_id: Option<RuntimeNodeId>,
    output_dirty: bool,
    reparented: bool,
}

pub(super) struct LayoutTree {
    nodes: Arena<LayoutNode>,
    outputs: Vec<NodeId>,
}

impl LayoutTree {
    pub fn new() -> Self {
        Self {
            nodes: Arena::new(),
            outputs: Vec::new(),
        }
    }

    fn get(&self, id: NodeId) -> Result<&LayoutNode> {
        self.nodes
            .get(Id::from_bits(u64::from(id)))
            .ok_or(TaffyError::InvalidInputNode(id))
    }

    fn get_mut(&mut self, id: NodeId) -> Result<&mut LayoutNode> {
        self.nodes
            .get_mut(Id::from_bits(u64::from(id)))
            .ok_or(TaffyError::InvalidInputNode(id))
    }

    pub fn new_leaf(&mut self, style: Style) -> Result<NodeId> {
        let id = self.nodes.insert_with(|_| LayoutNode {
            style,
            measure: None,
            parent: None,
            children: Vec::new(),
            cache: Cache::new(),
            unrounded: Layout::new(),
            rounded: Layout::new(),
            cumulative: Point::ZERO,
            runtime_id: None,
            output_dirty: false,
            reparented: false,
        });
        Ok(NodeId::from(id.to_bits()))
    }

    pub fn set_runtime_id(&mut self, id: NodeId, runtime_id: RuntimeNodeId) {
        self.get_mut(id)
            .expect("mounted layout node exists")
            .runtime_id = Some(runtime_id);
    }

    fn invalidate(&mut self, id: NodeId) -> Result<()> {
        let mut current = Some(id);
        while let Some(id) = current {
            let node = self.get_mut(id)?;
            node.cache.clear();
            current = node.parent;
        }
        Ok(())
    }

    pub fn set_style(&mut self, id: NodeId, style: Style) -> Result<()> {
        self.get_mut(id)?.style = style;
        self.invalidate(id)
    }

    pub fn set_node_context(
        &mut self,
        id: NodeId,
        measure: Option<crate::MeasureFn>,
    ) -> Result<()> {
        self.get_mut(id)?.measure = measure;
        self.invalidate(id)
    }

    pub fn set_children(&mut self, parent: NodeId, children: &[NodeId]) -> Result<()> {
        self.get(parent)?;
        for &child in children {
            self.get(child)?;
        }
        for &child in children {
            if self.get(child)?.parent != Some(parent) {
                self.get_mut(child)?.reparented = true;
                self.queue_output(child);
            }
        }
        let mut previous = std::mem::take(&mut self.get_mut(parent)?.children);
        for &child in &previous {
            self.get_mut(child)?.parent = None;
        }
        for &child in children {
            self.get_mut(child)?.parent = Some(parent);
        }
        previous.clear();
        previous.extend_from_slice(children);
        self.get_mut(parent)?.children = previous;
        self.invalidate(parent)
    }

    pub fn children(&self, id: NodeId) -> Result<Vec<NodeId>> {
        Ok(self.get(id)?.children.clone())
    }

    pub fn remove(&mut self, id: NodeId) -> Result<()> {
        let parent = self.get(id)?.parent;
        if let Some(parent) = parent {
            self.get_mut(parent)?.children.retain(|&child| child != id);
            self.invalidate(parent)?;
        }
        let removed = self
            .nodes
            .remove(Id::from_bits(u64::from(id)))
            .expect("validated layout node");
        for child in removed.children {
            self.get_mut(child)?.parent = None;
        }
        Ok(())
    }

    pub fn layout(&self, id: NodeId) -> Result<&Layout> {
        Ok(&self.get(id)?.rounded)
    }

    fn queue_output(&mut self, id: NodeId) {
        let node = self.get_mut(id).expect("layout node exists");
        if !node.output_dirty {
            node.output_dirty = true;
            self.outputs.push(id);
        }
    }

    pub fn compute_layout(
        &mut self,
        root: NodeId,
        available: Size<AvailableSpace>,
    ) -> Vec<RuntimeNodeId> {
        taffy::compute_root_layout(self, root, available);
        let mut paths: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut visited = HashSet::new();
        for id in std::mem::take(&mut self.outputs) {
            let Ok(node) = self.get_mut(id) else {
                continue;
            };
            node.output_dirty = false;
            let mut current = id;
            while let Some(parent) = self.get(current).expect("layout ancestor exists").parent {
                if !visited.insert(current) {
                    break;
                }
                paths.entry(parent).or_default().push(current);
                current = parent;
            }
        }
        let mut changed = Vec::new();
        let mut stack = vec![(root, Point::ZERO)];
        while let Some((id, parent_origin)) = stack.pop() {
            #[cfg(feature = "perf-metrics")]
            crate::metrics::record(|m| m.layout_nodes_rounded += 1);
            let node = self.get_mut(id).expect("layout node exists");
            let cumulative = Point {
                x: parent_origin.x + node.unrounded.location.x,
                y: parent_origin.y + node.unrounded.location.y,
            };
            let moved = cumulative != node.cumulative;
            node.cumulative = cumulative;
            let rounded = round(node.unrounded, cumulative);
            if node.rounded != rounded || moved || node.reparented {
                node.rounded = rounded;
                changed.extend(node.runtime_id);
            }
            node.reparented = false;
            if moved {
                stack.extend(node.children.iter().map(|&child| (child, cumulative)));
            } else if let Some(children) = paths.remove(&id) {
                stack.extend(children.into_iter().map(|child| (child, cumulative)));
            }
        }
        changed
    }
}

fn round(mut layout: Layout, origin: Point<f32>) -> Layout {
    let edge = |start: f32, length: f32| (start + length).round() - start.round();
    layout.location.x = layout.location.x.round();
    layout.location.y = layout.location.y.round();
    layout.border.left = edge(origin.x, layout.border.left);
    let far_edge =
        |start: f32, size: f32, width: f32| (start + size).round() - (start + size - width).round();
    layout.border.right = far_edge(origin.x, layout.size.width, layout.border.right);
    layout.border.top = edge(origin.y, layout.border.top);
    layout.border.bottom = far_edge(origin.y, layout.size.height, layout.border.bottom);
    layout.padding.left = edge(origin.x, layout.padding.left);
    layout.padding.right = far_edge(origin.x, layout.size.width, layout.padding.right);
    layout.padding.top = edge(origin.y, layout.padding.top);
    layout.padding.bottom = far_edge(origin.y, layout.size.height, layout.padding.bottom);
    layout.size.width = edge(origin.x, layout.size.width);
    layout.size.height = edge(origin.y, layout.size.height);
    layout.content_size.width = edge(origin.x, layout.content_size.width);
    layout.content_size.height = edge(origin.y, layout.content_size.height);
    layout.scrollbar_size.width = layout.scrollbar_size.width.round();
    layout.scrollbar_size.height = layout.scrollbar_size.height.round();
    layout
}

impl TraversePartialTree for LayoutTree {
    type ChildIter<'a> = std::iter::Copied<std::slice::Iter<'a, NodeId>>;

    fn child_ids(&self, id: NodeId) -> Self::ChildIter<'_> {
        self.get(id)
            .expect("layout node exists")
            .children
            .iter()
            .copied()
    }
    fn child_count(&self, id: NodeId) -> usize {
        self.get(id).expect("layout node exists").children.len()
    }
    fn get_child_id(&self, id: NodeId, index: usize) -> NodeId {
        self.get(id).expect("layout node exists").children[index]
    }
}

impl LayoutPartialTree for LayoutTree {
    type CoreContainerStyle<'a> = &'a Style;

    fn get_core_container_style(&self, id: NodeId) -> &Style {
        &self.get(id).expect("layout node exists").style
    }

    fn set_unrounded_layout(&mut self, id: NodeId, layout: &Layout) {
        let node = self.get_mut(id).expect("layout node exists");
        if node.unrounded != *layout {
            node.unrounded = *layout;
            self.queue_output(id);
        }
    }

    fn compute_child_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        if inputs.run_mode == RunMode::PerformHiddenLayout {
            return taffy::compute_hidden_layout(self, id);
        }
        taffy::compute_cached_layout(self, id, inputs, |tree, id, inputs| {
            match (
                tree.get(id).expect("layout node exists").style.display,
                tree.child_count(id) > 0,
            ) {
                (Display::None, _) => taffy::compute_hidden_layout(tree, id),
                (Display::Block, true) => taffy::compute_block_layout(tree, id, inputs),
                (Display::Flex, true) => taffy::compute_flexbox_layout(tree, id, inputs),
                (Display::Grid, true) => taffy::compute_grid_layout(tree, id, inputs),
                (_, false) => {
                    let node = tree.get_mut(id).expect("layout node exists");
                    taffy::compute_leaf_layout(inputs, &node.style, |known, available| match &node
                        .measure
                    {
                        Some(measure) => {
                            #[cfg(feature = "perf-metrics")]
                            crate::metrics::record(|m| m.measure_calls += 1);
                            measure(known, available)
                        }
                        None => Size::ZERO,
                    })
                }
            }
        })
    }
}

impl CacheTree for LayoutTree {
    fn cache_get(
        &self,
        id: NodeId,
        known: Size<Option<f32>>,
        available: Size<AvailableSpace>,
        mode: RunMode,
    ) -> Option<LayoutOutput> {
        self.get(id)
            .expect("layout node exists")
            .cache
            .get(known, available, mode)
    }
    fn cache_store(
        &mut self,
        id: NodeId,
        known: Size<Option<f32>>,
        available: Size<AvailableSpace>,
        mode: RunMode,
        output: LayoutOutput,
    ) {
        self.get_mut(id)
            .expect("layout node exists")
            .cache
            .store(known, available, mode, output);
    }
    fn cache_clear(&mut self, id: NodeId) {
        self.get_mut(id).expect("layout node exists").cache.clear();
    }
}

impl taffy::LayoutBlockContainer for LayoutTree {
    type BlockContainerStyle<'a> = &'a Style;
    type BlockItemStyle<'a> = &'a Style;
    fn get_block_container_style(&self, id: NodeId) -> &Style {
        self.get_core_container_style(id)
    }
    fn get_block_child_style(&self, id: NodeId) -> &Style {
        self.get_core_container_style(id)
    }
}

impl taffy::LayoutFlexboxContainer for LayoutTree {
    type FlexboxContainerStyle<'a> = &'a Style;
    type FlexboxItemStyle<'a> = &'a Style;
    fn get_flexbox_container_style(&self, id: NodeId) -> &Style {
        self.get_core_container_style(id)
    }
    fn get_flexbox_child_style(&self, id: NodeId) -> &Style {
        self.get_core_container_style(id)
    }
}

impl taffy::LayoutGridContainer for LayoutTree {
    type GridContainerStyle<'a> = &'a Style;
    type GridItemStyle<'a> = &'a Style;
    fn get_grid_container_style(&self, id: NodeId) -> &Style {
        self.get_core_container_style(id)
    }
    fn get_grid_child_style(&self, id: NodeId) -> &Style {
        self.get_core_container_style(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::style::{Dimension, FlexDirection, LengthPercentage, Position};
    use taffy::{prelude::TaffyMaxContent, TaffyTree};

    fn style(seed: u32) -> Style {
        let dimension = |value: u32| Dimension::Length(10.25 + (value % 70) as f32 * 0.35);
        Style {
            display: match seed % 13 {
                0 => Display::None,
                1..=3 => Display::Block,
                4..=6 => Display::Grid,
                _ => Display::Flex,
            },
            size: Size {
                width: if seed % 5 == 0 {
                    Dimension::Percent(0.25)
                } else {
                    dimension(seed)
                },
                height: if seed % 7 == 0 {
                    Dimension::Auto
                } else {
                    dimension(seed / 7)
                },
            },
            padding: taffy::geometry::Rect {
                left: LengthPercentage::Length(0.65),
                right: LengthPercentage::Length(0.65),
                top: LengthPercentage::Length(0.65),
                bottom: LengthPercentage::Length(0.65),
            },
            border: taffy::geometry::Rect {
                left: LengthPercentage::Length(1.15),
                right: LengthPercentage::Length(1.15),
                top: LengthPercentage::Length(1.15),
                bottom: LengthPercentage::Length(1.15),
            },
            gap: Size {
                width: LengthPercentage::Length(0.45),
                height: LengthPercentage::Length(1.25),
            },
            flex_direction: if seed % 2 == 0 {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            },
            position: if seed % 9 == 0 {
                Position::Absolute
            } else {
                Position::Relative
            },
            ..Default::default()
        }
    }

    fn assert_layouts(tree: &LayoutTree, reference: &TaffyTree, nodes: &[(NodeId, NodeId)]) {
        for &(id, expected) in nodes {
            assert_eq!(
                tree.layout(id).unwrap(),
                reference.layout(expected).unwrap(),
                "node {id:?}"
            );
        }
    }

    #[test]
    fn incremental_outputs_match_taffy_across_layout_algorithms_and_viewports() {
        let mut tree = LayoutTree::new();
        let mut reference = TaffyTree::new();
        let mut nodes = Vec::new();
        for index in 0..100 {
            let initial = style(index * 37);
            nodes.push((
                tree.new_leaf(initial.clone()).unwrap(),
                reference.new_leaf(initial).unwrap(),
            ));
        }
        for parent in 0..25 {
            let first = parent * 4 + 1;
            let end = (first + 4).min(nodes.len());
            if first < end {
                tree.set_children(
                    nodes[parent].0,
                    &nodes[first..end].iter().map(|n| n.0).collect::<Vec<_>>(),
                )
                .unwrap();
                reference
                    .set_children(
                        nodes[parent].1,
                        &nodes[first..end].iter().map(|n| n.1).collect::<Vec<_>>(),
                    )
                    .unwrap();
            }
        }
        let mut seed = 731_u32;
        for step in 0..250 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let index = seed as usize % nodes.len();
            let next = style(seed);
            tree.set_style(nodes[index].0, next.clone()).unwrap();
            reference.set_style(nodes[index].1, next).unwrap();
            let available = Size {
                width: AvailableSpace::Definite(160.3 + (step % 17) as f32 * 0.25),
                height: AvailableSpace::Definite(180.7 + (step % 23) as f32 * 0.15),
            };
            tree.compute_layout(nodes[0].0, available);
            reference.compute_layout(nodes[0].1, available).unwrap();
            assert_layouts(&tree, &reference, &nodes);
        }
    }

    #[test]
    fn reparenting_rounds_cached_descendants_at_their_new_cumulative_position() {
        let mut tree = LayoutTree::new();
        let mut reference = TaffyTree::new();
        let mut nodes = Vec::new();
        for index in 0..5 {
            let mut initial = style(11);
            initial.size = Size {
                width: Dimension::Length(25.25),
                height: Dimension::Length(30.35),
            };
            if index == 0 {
                initial.size.width = Dimension::Length(200.0);
            }
            nodes.push((
                tree.new_leaf(initial.clone()).unwrap(),
                reference.new_leaf(initial).unwrap(),
            ));
        }
        for (parent, children) in [(0, vec![1, 2]), (1, vec![3]), (3, vec![4])] {
            tree.set_children(
                nodes[parent].0,
                &children.iter().map(|&i| nodes[i].0).collect::<Vec<_>>(),
            )
            .unwrap();
            reference
                .set_children(
                    nodes[parent].1,
                    &children.iter().map(|&i| nodes[i].1).collect::<Vec<_>>(),
                )
                .unwrap();
        }
        tree.compute_layout(nodes[0].0, Size::MAX_CONTENT);
        reference
            .compute_layout(nodes[0].1, Size::MAX_CONTENT)
            .unwrap();
        assert_layouts(&tree, &reference, &nodes);
        tree.set_children(nodes[1].0, &[]).unwrap();
        reference.set_children(nodes[1].1, &[]).unwrap();
        tree.set_children(nodes[2].0, &[nodes[3].0]).unwrap();
        reference.set_children(nodes[2].1, &[nodes[3].1]).unwrap();
        tree.compute_layout(nodes[0].0, Size::MAX_CONTENT);
        reference
            .compute_layout(nodes[0].1, Size::MAX_CONTENT)
            .unwrap();
        assert_layouts(&tree, &reference, &nodes);
    }

    #[test]
    fn removing_pending_outputs_does_not_alias_a_reused_slot() {
        let mut tree = LayoutTree::new();
        let root = tree.new_leaf(style(11)).unwrap();
        let removed = tree.new_leaf(style(11)).unwrap();
        tree.set_children(root, &[removed]).unwrap();
        tree.remove(removed).unwrap();
        let replacement = tree.new_leaf(style(29)).unwrap();
        assert_ne!(removed, replacement);
        assert!(tree.layout(removed).is_err());
        tree.set_children(root, &[replacement]).unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT);
        assert!(tree.layout(replacement).unwrap().size.width > 0.0);
    }
}
