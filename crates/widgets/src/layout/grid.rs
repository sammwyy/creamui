use super::{layout_container_methods, shrinkable, Justify, StyleExt};
use creamui_core::layout::{
    Dimension, Display, GridAutoFlow, GridPlacement, GridTrackRepetition,
    NonRepeatedTrackSizingFunction, Style, TaffyGridLine, TrackSizingFunction,
};
use creamui_core::{BoxedWidget, Painter, Rect, Widget};

/// A CSS grid track. Use [`Track::fr`] for proportional space, [`Track::px`]
/// for a fixed track, and [`Track::minmax`] for a track that's at least
/// `min` px wide but shares any remaining space like an `fr` track — the
/// building block of a responsive `repeat(auto-fit, minmax(..))` gallery
/// (see [`Grid::auto_fit_columns`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    Auto,
    Px(f32),
    Fr(f32),
    MinMax { min: f32, max_fr: f32 },
}

impl Track {
    pub const fn auto() -> Self {
        Self::Auto
    }

    pub const fn px(value: f32) -> Self {
        Self::Px(value)
    }

    pub const fn fr(value: f32) -> Self {
        Self::Fr(value)
    }

    /// A track that never shrinks below `min` px, then shares remaining
    /// space as `max_fr` fractional units — CSS's `minmax(min, max_fr fr)`.
    pub const fn minmax(min: f32, max_fr: f32) -> Self {
        Self::MinMax { min, max_fr }
    }

    fn sizing(self) -> NonRepeatedTrackSizingFunction {
        match self {
            Track::Auto => creamui_core::layout::auto(),
            Track::Px(value) => creamui_core::layout::length(value),
            Track::Fr(value) => creamui_core::layout::fr(value),
            Track::MinMax { min, max_fr } => creamui_core::layout::minmax(
                creamui_core::layout::length(min),
                creamui_core::layout::fr(max_fr),
            ),
        }
    }
}

/// Automatic placement order for a [`Grid`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

impl From<GridFlow> for GridAutoFlow {
    fn from(value: GridFlow) -> Self {
        match value {
            GridFlow::Row => Self::Row,
            GridFlow::Column => Self::Column,
            GridFlow::RowDense => Self::RowDense,
            GridFlow::ColumnDense => Self::ColumnDense,
        }
    }
}

/// An unstyled CSS grid container, equivalent to `<div style="display: grid">`.
///
/// ```
/// use creamui_widgets::layout::{Grid, Track};
///
/// let dashboard = Grid::new()
///     .columns(3)
///     .gap(16.0)
///     .template_rows([Track::px(48.0), Track::fr(1.0)]);
///
/// // A responsive card gallery, no manual column-count math required:
/// // as many 180px-or-wider columns as fit, sharing the rest of the width.
/// let gallery = Grid::new().auto_fit_columns(180.0).gap(16.0);
/// ```
pub struct Grid {
    inner: crate::raw::RawView,
}
impl_styled_inner!(Grid);

impl Grid {
    /// Creates an empty grid. Configure columns/rows with [`Grid::columns`],
    /// [`Grid::rows`], or explicit track templates.
    pub fn new() -> Self {
        Self {
            inner: crate::raw::RawView::new(shrinkable(Style {
                display: Display::Grid,
                ..Default::default()
            })),
        }
    }

    /// Creates `count` equal `1fr` columns.
    pub fn columns(mut self, count: usize) -> Self {
        self.inner.style.grid_template_columns = equal_tracks(count);
        self
    }

    /// Creates `count` equal `1fr` rows.
    pub fn rows(mut self, count: usize) -> Self {
        self.inner.style.grid_template_rows = equal_tracks(count);
        self
    }

    /// Sets an explicit CSS-like `grid-template-columns` list.
    pub fn template_columns(mut self, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.inner.style.grid_template_columns = tracks
            .into_iter()
            .map(|track| TrackSizingFunction::Single(track.sizing()))
            .collect();
        self
    }

    /// Sets an explicit CSS-like `grid-template-rows` list.
    pub fn template_rows(mut self, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.inner.style.grid_template_rows = tracks
            .into_iter()
            .map(|track| TrackSizingFunction::Single(track.sizing()))
            .collect();
        self
    }

    /// A responsive column template equivalent to CSS's
    /// `repeat(auto-fit, minmax(min, 1fr))`: Taffy generates as many
    /// `min`-px-or-wider columns as fit the grid's own available width, and
    /// the columns share whatever's left over evenly — collapsing empty
    /// tracks (and their gaps) if the content doesn't fill a whole row.
    ///
    /// This replaces manually computing a column count from a measured
    /// pixel width every frame: the same layout that a resize would have
    /// required recomputing by hand is instead resolved natively during
    /// layout, and only actually recomputed when the available width
    /// changes.
    pub fn auto_fit_columns(mut self, min: f32) -> Self {
        self.inner.style.grid_template_columns =
            auto_repeat_tracks(GridTrackRepetition::AutoFit, min);
        self
    }

    /// Like [`Grid::auto_fit_columns`], but keeps empty tracks (and their
    /// gaps) around instead of collapsing them — CSS grid's `auto-fill` vs
    /// `auto-fit` distinction.
    pub fn auto_fill_columns(mut self, min: f32) -> Self {
        self.inner.style.grid_template_columns =
            auto_repeat_tracks(GridTrackRepetition::AutoFill, min);
        self
    }

    /// Sets equal row and column gaps.
    pub fn gap(mut self, value: f32) -> Self {
        self.inner.style = self.inner.style.gap(value);
        self
    }

    /// Sets horizontal (`column-gap`) spacing.
    pub fn gap_x(mut self, value: f32) -> Self {
        self.inner.style = self.inner.style.gap_x(value);
        self
    }

    /// Sets vertical (`row-gap`) spacing.
    pub fn gap_y(mut self, value: f32) -> Self {
        self.inner.style = self.inner.style.gap_y(value);
        self
    }

    /// Sets `justify-content` on the inline axis.
    pub fn justify(mut self, value: Justify) -> Self {
        self.inner.style = self.inner.style.justify(value);
        self
    }

    /// Sets `align-content` on the block axis.
    pub fn align_content(mut self, value: Justify) -> Self {
        self.inner.style = self.inner.style.align_content(value.into());
        self
    }

    /// Sets the automatic grid placement order.
    pub fn flow(mut self, value: GridFlow) -> Self {
        self.inner.style.grid_auto_flow = value.into();
        self
    }

    layout_container_methods!();
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Grid {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

/// A positioned child of a [`Grid`]. Grid lines are 1-indexed, matching CSS.
pub struct GridItem {
    style: Style,
    children: Vec<BoxedWidget>,
}

impl GridItem {
    /// Creates an automatically placed grid item.
    pub fn new() -> Self {
        Self {
            style: shrinkable(Style {
                display: Display::Block,
                ..Default::default()
            }),
            children: vec![],
        }
    }

    /// Places the item at a 1-indexed `(column, row)` cell.
    pub fn at(mut self, column: i16, row: i16) -> Self {
        self.style = self.style.grid_cell(column, row);
        self
    }

    /// Sets the starting 1-indexed grid column.
    pub fn column(mut self, column: i16) -> Self {
        self.style.grid_column.start = GridPlacement::from_line_index(column);
        self
    }

    /// Sets the starting 1-indexed grid row.
    pub fn row(mut self, row: i16) -> Self {
        self.style.grid_row.start = GridPlacement::from_line_index(row);
        self
    }

    /// Makes the item span `count` columns.
    pub fn column_span(mut self, count: u16) -> Self {
        self.style.grid_column.end = GridPlacement::Span(count);
        self
    }

    /// Makes the item span `count` rows.
    pub fn row_span(mut self, count: u16) -> Self {
        self.style.grid_row.end = GridPlacement::Span(count);
        self
    }

    /// Sets a minimum pixel width, opting back into "never shrink below
    /// this width" — grid items already shrink to `0` by default (see the
    /// module docs), so this is only needed to keep one item from
    /// shrinking as readily as its siblings.
    pub fn min_width(mut self, value: f32) -> Self {
        self.style.min_size.width = Dimension::Length(value);
        self
    }

    /// Sets a minimum pixel height; see [`GridItem::min_width`].
    pub fn min_height(mut self, value: f32) -> Self {
        self.style.min_size.height = Dimension::Length(value);
        self
    }

    /// Adds the item's content.
    pub fn child(mut self, child: BoxedWidget) -> Self {
        self.children.push(child);
        self
    }

    /// Adds all item content at once.
    pub fn with_children(mut self, children: Vec<BoxedWidget>) -> Self {
        self.children = children;
        self
    }
}

impl Default for GridItem {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for GridItem {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }
}

fn equal_tracks(count: usize) -> Vec<TrackSizingFunction> {
    let track: NonRepeatedTrackSizingFunction = creamui_core::layout::fr(1.0f32);
    vec![TrackSizingFunction::Single(track); count]
}

/// A single `repeat(auto-fit | auto-fill, minmax(min, 1fr))` track group —
/// the whole point of `repeat`: Taffy decides how many copies fit at
/// layout time instead of the caller precomputing a column count.
fn auto_repeat_tracks(repetition: GridTrackRepetition, min: f32) -> Vec<TrackSizingFunction> {
    vec![TrackSizingFunction::Repeat(
        repetition,
        vec![creamui_core::layout::minmax(
            creamui_core::layout::length(min),
            creamui_core::layout::fr(1.0f32),
        )],
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::layout::{AvailableSpace, LengthPercentage};

    #[test]
    fn grid_expresses_tracks_and_item_placement() {
        let style = Grid::new()
            .columns(2)
            .template_rows([Track::Px(40.0), Track::Fr(1.0)])
            .gap(12.0)
            .style();
        let item = GridItem::new()
            .at(2, 1)
            .column_span(2)
            .row_span(3)
            .min_width(0.0)
            .style();

        assert_eq!(style.display, creamui_core::layout::Display::Grid);
        assert_eq!(style.grid_template_columns.len(), 2);
        assert_eq!(style.grid_template_rows.len(), 2);
        assert_eq!(style.gap.width, LengthPercentage::Length(12.0));
        assert_eq!(item.grid_column.start, GridPlacement::from_line_index(2));
        assert_eq!(item.grid_column.end, GridPlacement::Span(2));
        assert_eq!(item.grid_row.start, GridPlacement::from_line_index(1));
        assert_eq!(item.grid_row.end, GridPlacement::Span(3));
        assert_eq!(item.min_size.width, Dimension::Length(0.0));
    }

    #[test]
    fn auto_fit_columns_resolves_the_column_count_from_available_width() {
        let mut tree = creamui_core::layout::TaffyTree::<()>::new();
        let child_style = shrinkable(Style::default());
        let children: Vec<_> = (0..6)
            .map(|_| tree.new_leaf(child_style.clone()).unwrap())
            .collect();
        let root_style = Style {
            size: crate::layout::fixed(400.0, 200.0),
            ..Grid::new().auto_fit_columns(180.0).gap(20.0).style().layout
        };
        let root = tree.new_with_children(root_style, &children).unwrap();
        tree.compute_layout(
            root,
            creamui_core::layout::Size {
                width: AvailableSpace::Definite(400.0),
                height: AvailableSpace::Definite(200.0),
            },
        )
        .unwrap();

        // (400 - 20 gap) / (180 + 20) rounds down to 2 columns of ~190px.
        let first = tree.layout(children[0]).unwrap();
        let second = tree.layout(children[1]).unwrap();
        let third = tree.layout(children[2]).unwrap();
        assert_eq!(first.location.x, 0.0);
        assert!(second.location.x > first.location.x);
        assert_eq!(
            third.location.y > first.location.y,
            true,
            "a third column shouldn't fit, so the third card wraps to row 2"
        );
    }
}
