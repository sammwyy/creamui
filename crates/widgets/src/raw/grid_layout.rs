use super::*;
use creamui_core::layout::{Dimension, LengthPercentageAuto, Position};
use creamui_reactive::Signal;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridCell {
    pub id: String,
    pub column: usize,
    pub row: usize,
    pub columns: usize,
    pub rows: usize,
}

impl GridCell {
    pub fn new(id: impl Into<String>, column: usize, row: usize) -> Self {
        Self {
            id: id.into(),
            column,
            row,
            columns: 1,
            rows: 1,
        }
    }

    pub fn span(mut self, columns: usize, rows: usize) -> Self {
        self.columns = columns.max(1);
        self.rows = rows.max(1);
        self
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.column < other.column + other.columns
            && other.column < self.column + self.columns
            && self.row < other.row + other.rows
            && other.row < self.row + self.rows
    }
}

#[derive(Clone)]
pub struct GridController {
    cells: Signal<Vec<GridCell>>,
    bounds: Rc<Cell<Rect>>,
}

impl GridController {
    pub fn new(cells: Vec<GridCell>) -> Self {
        Self {
            cells: Signal::new(cells),
            bounds: Rc::new(Cell::new(Rect::default())),
        }
    }

    pub fn cells(&self) -> Vec<GridCell> {
        self.cells.get()
    }

    pub fn move_cell(&self, id: &str, column: usize, row: usize, total_columns: usize) -> bool {
        let cells = self.cells.peek();
        let Some(index) = cells.iter().position(|cell| cell.id == id) else {
            return false;
        };
        let mut candidate = cells[index].clone();
        candidate.column = column.min(total_columns.max(1).saturating_sub(candidate.columns));
        candidate.row = row.min(1000);
        if cells
            .iter()
            .enumerate()
            .any(|(other, cell)| other != index && candidate.overlaps(cell))
        {
            return false;
        }
        if candidate != cells[index] {
            self.cells.update(|cells| cells[index] = candidate);
        }
        true
    }
}

struct RawGridItem {
    style: creamui_core::Style,
    content: Option<BoxedWidget>,
    controller: GridController,
    id: String,
    columns: usize,
    row_height: f32,
    locked: bool,
}

impl Widget for RawGridItem {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.content.take().into_iter().collect()
    }

    fn focusable(&self) -> bool {
        !self.locked
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        if self.locked {
            return None;
        }
        let controller = self.controller.clone();
        let id = self.id.clone();
        let columns = self.columns;
        Some(Rc::new(move |input| {
            let Some(cell) = controller
                .cells
                .peek()
                .into_iter()
                .find(|cell| cell.id == id)
            else {
                return;
            };
            let (column, row) = match input.key {
                Key::Left => (cell.column.saturating_sub(1), cell.row),
                Key::Right => (cell.column + 1, cell.row),
                Key::Up => (cell.column, cell.row.saturating_sub(1)),
                Key::Down => (cell.column, cell.row + 1),
                _ => return,
            };
            controller.move_cell(&id, column, row, columns);
        }))
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        if self.locked {
            return None;
        }
        let controller = self.controller.clone();
        let id = self.id.clone();
        let columns = self.columns;
        let row_height = self.row_height;
        Some(Rc::new(move |point, rect| {
            let bounds = controller.bounds.get();
            if bounds.width <= 0.0 {
                return;
            }
            let x = (rect.x + point.x - bounds.x).max(0.0);
            let y = (rect.y + point.y - bounds.y).max(0.0);
            let column = (x / bounds.width * columns as f32).floor() as usize;
            let row = (y / row_height).floor() as usize;
            controller.move_cell(&id, column, row, columns);
        }))
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        (!self.locked).then_some(CursorIcon::Pointer)
    }
}

pub struct RawGridLayout {
    pub style: creamui_core::Style,
    controller: GridController,
    items: Vec<(String, BoxedWidget)>,
    columns: usize,
    row_height: f32,
    locked: bool,
}

impl RawGridLayout {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        controller: &GridController,
        columns: usize,
    ) -> Self {
        let _ = controller.cells();
        Self {
            style: style.into(),
            controller: controller.clone(),
            items: Vec::new(),
            columns: columns.max(1),
            row_height: 80.0,
            locked: false,
        }
    }

    pub fn item(mut self, id: impl Into<String>, content: BoxedWidget) -> Self {
        self.items.push((id.into(), content));
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(1.0);
        self
    }

    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }
}

impl Widget for RawGridLayout {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        if let Some(rows) = self
            .controller
            .cells()
            .iter()
            .map(|cell| cell.row + cell.rows)
            .max()
        {
            style.layout.size.height = Dimension::Length(rows as f32 * self.row_height);
        }
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn reports_layout(&self) -> bool {
        true
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.controller.bounds.set(rect);
        false
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        let cells = self.controller.cells();
        std::mem::take(&mut self.items)
            .into_iter()
            .filter_map(|(id, content)| {
                let cell = cells.iter().find(|cell| cell.id == id)?;
                let style = Style {
                    position: Position::Absolute,
                    inset: creamui_core::layout::Rect {
                        left: LengthPercentageAuto::Percent(
                            cell.column as f32 / self.columns as f32,
                        ),
                        top: LengthPercentageAuto::Length(cell.row as f32 * self.row_height),
                        right: LengthPercentageAuto::Auto,
                        bottom: LengthPercentageAuto::Auto,
                    },
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(cell.columns as f32 / self.columns as f32),
                        height: Dimension::Length(cell.rows as f32 * self.row_height),
                    },
                    ..Default::default()
                };
                Some(Box::new(RawGridItem {
                    style: style.into(),
                    content: Some(content),
                    controller: self.controller.clone(),
                    id,
                    columns: self.columns,
                    row_height: self.row_height,
                    locked: self.locked,
                }) as BoxedWidget)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_rejects_collisions_and_moves_items_within_columns() {
        let controller = GridController::new(vec![
            GridCell::new("one", 0, 0).span(2, 1),
            GridCell::new("two", 2, 0),
        ]);
        assert!(!controller.move_cell("one", 1, 0, 3));
        assert!(controller.move_cell("one", 20, 1, 3));
        assert_eq!(controller.cells()[0].column, 1);
        assert_eq!(controller.cells()[0].row, 1);
    }

    #[test]
    fn grid_mounts_mapped_item_slots() {
        let controller = GridController::new(vec![GridCell::new("one", 0, 0)]);
        let mut grid = RawGridLayout::new(Style::default(), &controller, 2)
            .item("one", Box::new(RawView::new(Style::default())))
            .item("unknown", Box::new(RawView::new(Style::default())));
        assert_eq!(grid.children().len(), 1);
    }
}
