use super::*;
use crate::{RawVirtualList, ScrollController, VirtualListState};
use creamui_core::layout::Dimension;
use creamui_reactive::Signal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Context,
    Addition,
    Deletion,
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffView {
    Unified,
    SideBySide,
}

#[derive(Clone)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub old_number: Option<usize>,
    pub new_number: Option<usize>,
    pub text: String,
}

impl DiffLine {
    pub fn new(
        kind: DiffKind,
        old_number: Option<usize>,
        new_number: Option<usize>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            old_number,
            new_number,
            text: text.into(),
        }
    }
}

#[derive(Clone)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

impl DiffHunk {
    pub fn new(header: impl Into<String>) -> Self {
        Self {
            header: header.into(),
            lines: Vec::new(),
        }
    }

    pub fn line(mut self, line: DiffLine) -> Self {
        self.lines.push(line);
        self
    }
}

#[derive(Clone)]
pub struct DiffController {
    view: Signal<DiffView>,
    selected: Signal<Option<(usize, usize)>>,
}

impl DiffController {
    pub fn new() -> Self {
        Self {
            view: Signal::new(DiffView::Unified),
            selected: Signal::new(None),
        }
    }

    pub fn view(&self) -> DiffView {
        self.view.get()
    }

    pub fn set_view(&self, view: DiffView) {
        self.view.set_if_changed(view);
    }

    pub fn selected(&self) -> Option<(usize, usize)> {
        self.selected.get()
    }

    pub fn select(&self, selection: Option<(usize, usize)>) {
        self.selected.set_if_changed(selection);
    }
}

impl Default for DiffController {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct DiffColors {
    pub context: Color,
    pub addition: Color,
    pub deletion: Color,
    pub conflict: Color,
    pub selected: Color,
    pub header: Color,
    pub text: Color,
    pub gutter: Color,
}

impl Default for DiffColors {
    fn default() -> Self {
        Self {
            context: Color::rgb(245, 245, 245),
            addition: Color::rgb(222, 247, 225),
            deletion: Color::rgb(255, 225, 225),
            conflict: Color::rgb(255, 241, 197),
            selected: Color::rgb(210, 225, 250),
            header: Color::rgb(230, 230, 230),
            text: Color::rgb(28, 28, 28),
            gutter: Color::rgb(100, 100, 100),
        }
    }
}

#[derive(Clone)]
enum DiffRow {
    Header(String),
    Unified(usize, usize, DiffLine),
    Split(
        Option<(usize, usize, DiffLine)>,
        Option<(usize, usize, DiffLine)>,
    ),
}

fn flatten(hunks: &[DiffHunk], view: DiffView) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    for (hunk_index, hunk) in hunks.iter().enumerate() {
        rows.push(DiffRow::Header(hunk.header.clone()));
        if view == DiffView::Unified {
            rows.extend(
                hunk.lines
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(index, line)| DiffRow::Unified(hunk_index, index, line)),
            );
            continue;
        }
        let mut index = 0;
        while index < hunk.lines.len() {
            let line = &hunk.lines[index];
            if matches!(line.kind, DiffKind::Context | DiffKind::Conflict) {
                rows.push(DiffRow::Split(
                    Some((hunk_index, index, line.clone())),
                    Some((hunk_index, index, line.clone())),
                ));
                index += 1;
                continue;
            }
            let start = index;
            while index < hunk.lines.len() && hunk.lines[index].kind == DiffKind::Deletion {
                index += 1;
            }
            let deleted = start..index;
            let start = index;
            while index < hunk.lines.len() && hunk.lines[index].kind == DiffKind::Addition {
                index += 1;
            }
            let added = start..index;
            if deleted.is_empty() && added.is_empty() {
                index += 1;
                continue;
            }
            for offset in 0..deleted.len().max(added.len()) {
                let left = deleted.clone().nth(offset);
                let right = added.clone().nth(offset);
                rows.push(DiffRow::Split(
                    left.map(|i| (hunk_index, i, hunk.lines[i].clone())),
                    right.map(|i| (hunk_index, i, hunk.lines[i].clone())),
                ));
            }
        }
    }
    rows
}

struct DiffRowWidget {
    row: DiffRow,
    colors: DiffColors,
    controller: DiffController,
    selected: Option<(usize, usize)>,
    height: f32,
    width: Rc<Cell<f32>>,
}

impl DiffRowWidget {
    fn color(&self, kind: DiffKind) -> Color {
        match kind {
            DiffKind::Context => self.colors.context,
            DiffKind::Addition => self.colors.addition,
            DiffKind::Deletion => self.colors.deletion,
            DiffKind::Conflict => self.colors.conflict,
        }
    }

    fn cell(
        &self,
        painter: &mut dyn Painter,
        rect: Rect,
        line: Option<(usize, usize, &DiffLine)>,
        left: bool,
    ) {
        let Some((hunk, index, line)) = line else {
            painter.fill_rect(rect, self.colors.context, 0.0);
            return;
        };
        let selected = self.selected == Some((hunk, index));
        painter.fill_rect(
            rect,
            if selected {
                self.colors.selected
            } else {
                self.color(line.kind)
            },
            0.0,
        );
        let number = if left {
            line.old_number
        } else {
            line.new_number
        };
        painter.fill_text(
            Rect {
                x: rect.x + 2.0,
                y: rect.y,
                width: 24.0,
                height: rect.height,
            },
            &number.map_or(String::new(), |number| number.to_string()),
            self.colors.gutter,
            11.0,
            TextAlign::End,
        );
        painter.fill_text_font(
            Rect {
                x: rect.x + 30.0,
                y: rect.y,
                width: (rect.width - 34.0).max(0.0),
                height: rect.height,
            },
            &line.text,
            self.colors.text,
            12.0,
            TextAlign::Start,
            Some("monospace"),
            false,
            false,
        );
    }
}

impl Widget for DiffRowWidget {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(self.height),
            },
            flex_shrink: 0.0,
            ..Default::default()
        }
        .into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        match &self.row {
            DiffRow::Header(text) => {
                painter.fill_rect(rect, self.colors.header, 0.0);
                painter.fill_text_font(
                    Rect {
                        x: rect.x + 8.0,
                        width: (rect.width - 8.0).max(0.0),
                        ..rect
                    },
                    text,
                    self.colors.text,
                    12.0,
                    TextAlign::Start,
                    Some("monospace"),
                    false,
                    false,
                );
            }
            DiffRow::Unified(hunk, index, line) => self.cell(
                painter,
                rect,
                Some((*hunk, *index, line)),
                line.kind != DiffKind::Addition,
            ),
            DiffRow::Split(left, right) => {
                let half = rect.width / 2.0;
                self.cell(
                    painter,
                    Rect {
                        width: half,
                        ..rect
                    },
                    left.as_ref()
                        .map(|(hunk, index, line)| (*hunk, *index, line)),
                    true,
                );
                self.cell(
                    painter,
                    Rect {
                        x: rect.x + half,
                        width: half,
                        ..rect
                    },
                    right
                        .as_ref()
                        .map(|(hunk, index, line)| (*hunk, *index, line)),
                    false,
                );
                painter.fill_rect(
                    Rect {
                        x: rect.x + half,
                        width: 1.0,
                        ..rect
                    },
                    self.colors.gutter,
                    0.0,
                );
            }
        }
    }

    fn clips_children(&self) -> bool {
        true
    }

    fn reports_layout(&self) -> bool {
        true
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.width.set(rect.width);
        false
    }

    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        let controller = self.controller.clone();
        let row = self.row.clone();
        let width = self.width.clone();
        Some(Rc::new(move |point| {
            let selection = match &row {
                DiffRow::Header(_) => return,
                DiffRow::Unified(hunk, index, _) => Some((*hunk, *index)),
                DiffRow::Split(left, right) => {
                    let selected = if point.x < width.get() / 2.0 {
                        left.as_ref().or(right.as_ref())
                    } else {
                        right.as_ref().or(left.as_ref())
                    };
                    selected.map(|(hunk, index, _)| (*hunk, *index))
                }
            };
            controller.select(selection);
        }))
    }
}

pub struct RawDiff {
    pub style: creamui_core::Style,
    inner: RawVirtualList,
    colors: Rc<RefCell<DiffColors>>,
}

impl RawDiff {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        controller: &DiffController,
        state: VirtualListState,
        scroll: ScrollController,
        viewport_height: f32,
        hunks: &[DiffHunk],
    ) -> Self {
        let mut style: creamui_core::Style = style.into();
        style.layout.flex_direction = creamui_core::layout::FlexDirection::Column;
        let colors = Rc::new(RefCell::new(DiffColors::default()));
        let rows = Rc::new(flatten(hunks, controller.view()));
        if state.len() != rows.len() {
            state.set_item_count(rows.len(), 26.0);
        }
        let selected = controller.selected();
        let controller = controller.clone();
        let row_colors = colors.clone();
        let inner = RawVirtualList::new(
            style.clone(),
            state,
            scroll,
            viewport_height,
            move |index| {
                let row = rows[index].clone();
                Box::new(DiffRowWidget {
                    height: if matches!(row, DiffRow::Header(_)) {
                        28.0
                    } else {
                        26.0
                    },
                    row,
                    colors: row_colors.borrow().clone(),
                    controller: controller.clone(),
                    selected,
                    width: Rc::new(Cell::new(0.0)),
                })
            },
        );
        Self {
            style,
            inner,
            colors,
        }
    }

    pub fn colors(self, colors: DiffColors) -> Self {
        *self.colors.borrow_mut() = colors;
        self
    }
}

impl Widget for RawDiff {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_aligns_replacement_blocks_and_conflicts() {
        let hunks = vec![DiffHunk::new("@@ -1,3 +1,3 @@")
            .line(DiffLine::new(DiffKind::Deletion, Some(1), None, "old"))
            .line(DiffLine::new(DiffKind::Deletion, Some(2), None, "old 2"))
            .line(DiffLine::new(DiffKind::Addition, None, Some(1), "new"))
            .line(DiffLine::new(DiffKind::Conflict, Some(3), Some(2), "<<<<"))];
        let rows = flatten(&hunks, DiffView::SideBySide);
        assert_eq!(rows.len(), 4);
        assert!(matches!(&rows[1], DiffRow::Split(Some(_), Some(_))));
        assert!(matches!(&rows[2], DiffRow::Split(Some(_), None)));
        assert!(matches!(&rows[3], DiffRow::Split(Some(_), Some(_))));
    }

    #[test]
    fn virtual_diff_tracks_view_and_selection() {
        let controller = DiffController::new();
        let state = VirtualListState::new(0, 26.0);
        let hunks = vec![DiffHunk::new("Header")
            .line(DiffLine::new(DiffKind::Deletion, Some(1), None, "before"))
            .line(DiffLine::new(DiffKind::Addition, None, Some(1), "after"))];
        let mut diff = RawDiff::new(
            Style::default(),
            &controller,
            state.clone(),
            ScrollController::new(0.0),
            100.0,
            &hunks,
        );
        assert_eq!(state.len(), 3);
        assert_eq!(diff.children().len(), 1);
        controller.set_view(DiffView::SideBySide);
        let _diff = RawDiff::new(
            Style::default(),
            &controller,
            state.clone(),
            ScrollController::new(0.0),
            100.0,
            &hunks,
        );
        assert_eq!(state.len(), 2);
    }
}
