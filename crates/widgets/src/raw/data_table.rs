use super::*;
use crate::{RawVirtualTable, ScrollController, TableColumn, VirtualListState};
use creamui_reactive::Signal;
use std::cmp::Ordering;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone)]
pub struct DataTableController {
    column_count: usize,
    sort: Signal<Option<(usize, SortDirection)>>,
    filter: Signal<String>,
    selected: Signal<Vec<usize>>,
    order: Signal<Vec<usize>>,
    hidden: Signal<Vec<usize>>,
}

impl DataTableController {
    pub fn new(column_count: usize) -> Self {
        Self {
            column_count,
            sort: Signal::new(None),
            filter: Signal::new(String::new()),
            selected: Signal::new(Vec::new()),
            order: Signal::new((0..column_count).collect()),
            hidden: Signal::new(Vec::new()),
        }
    }

    pub fn sort(&self) -> Option<(usize, SortDirection)> {
        self.sort.get()
    }

    pub fn sort_by(&self, column: usize) {
        if column < self.column_count {
            let next = match self.sort.peek() {
                Some((index, SortDirection::Ascending)) if index == column => {
                    Some((column, SortDirection::Descending))
                }
                Some((index, SortDirection::Descending)) if index == column => None,
                _ => Some((column, SortDirection::Ascending)),
            };
            self.sort.set(next);
        }
    }

    pub fn set_filter(&self, filter: impl Into<String>) {
        self.filter.set_if_changed(filter.into());
    }

    pub fn filter(&self) -> String {
        self.filter.get()
    }

    pub fn selected(&self) -> Vec<usize> {
        self.selected.get()
    }

    pub fn toggle_row(&self, row: usize) {
        self.selected.update(|rows| {
            if let Some(index) = rows.iter().position(|selected| *selected == row) {
                rows.remove(index);
            } else {
                rows.push(row);
            }
        });
    }

    pub fn set_column_visible(&self, column: usize, visible: bool) {
        if column >= self.column_count {
            return;
        }
        let hidden = self.hidden.peek();
        if hidden.contains(&column) == visible {
            self.hidden.update(|hidden| {
                if visible {
                    hidden.retain(|index| *index != column);
                } else {
                    hidden.push(column);
                }
            });
        }
    }

    pub fn visible_columns(&self) -> Vec<usize> {
        let hidden = self.hidden.get();
        self.order
            .get()
            .into_iter()
            .filter(|index| !hidden.contains(index))
            .collect()
    }

    pub fn reorder_column(&self, from: usize, to: usize) -> bool {
        if from >= self.column_count || to >= self.column_count {
            return false;
        }
        self.order.update(|order| {
            let item = order.remove(from);
            order.insert(to, item);
        });
        true
    }

    pub fn visible_rows(&self, rows: &[Vec<String>]) -> Vec<usize> {
        let filter = self.filter.get().to_lowercase();
        let mut indices: Vec<_> = rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                (filter.is_empty() || row.iter().any(|cell| cell.to_lowercase().contains(&filter)))
                    .then_some(index)
            })
            .collect();
        if let Some((column, direction)) = self.sort.get() {
            indices.sort_by(|left, right| {
                let first = rows[*left].get(column).map(String::as_str).unwrap_or("");
                let second = rows[*right].get(column).map(String::as_str).unwrap_or("");
                let order = match (first.parse::<f64>(), second.parse::<f64>()) {
                    (Ok(first), Ok(second)) => first.total_cmp(&second),
                    _ => first.to_lowercase().cmp(&second.to_lowercase()),
                };
                let order = if direction == SortDirection::Ascending {
                    order
                } else {
                    order.reverse()
                };
                if order == Ordering::Equal {
                    left.cmp(right)
                } else {
                    order
                }
            });
        }
        indices
    }
}

pub struct RawDataTable {
    inner: RawVirtualTable,
}

impl RawDataTable {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        style: impl Into<creamui_core::Style>,
        scroll: ScrollController,
        columns: Vec<TableColumn>,
        rows: Vec<Vec<String>>,
        controller: &DataTableController,
        state: VirtualListState,
        viewport_height: f32,
    ) -> Self {
        let visible_columns = controller.visible_columns();
        let visible_rows = controller.visible_rows(&rows);
        if state.len() != visible_rows.len() {
            state.set_item_count(visible_rows.len(), 32.0);
        }
        let selected = controller.selected();
        let selected_rows: HashSet<usize> = visible_rows
            .iter()
            .enumerate()
            .filter_map(|(display, original)| selected.contains(original).then_some(display))
            .collect();
        let projected_columns = visible_columns
            .iter()
            .filter_map(|index| {
                let mut column = columns.get(*index)?.clone();
                if let Some((sorted, direction)) = controller.sort() {
                    if sorted == *index {
                        column.label.push_str(match direction {
                            SortDirection::Ascending => " ^",
                            SortDirection::Descending => " v",
                        });
                    }
                }
                Some(column)
            })
            .collect();
        let rows = Rc::new(rows);
        let row_indices = Rc::new(visible_rows);
        let display_columns = Rc::new(visible_columns);
        let row_source = rows.clone();
        let selected_indices = row_indices.clone();
        let column_indices = display_columns.clone();
        let row_controller = controller.clone();
        let sort_controller = controller.clone();
        let inner = RawVirtualTable::new(
            style,
            scroll,
            projected_columns,
            state,
            viewport_height,
            move |index| {
                let row = &row_source[row_indices[index]];
                column_indices
                    .iter()
                    .map(|column| row.get(*column).cloned().unwrap_or_default())
                    .collect()
            },
        )
        .on_row_click(None, move |index| {
            if let Some(row) = selected_indices.get(index) {
                row_controller.toggle_row(*row);
            }
        })
        .customize(move |table| {
            table.selected_rows = Some(Rc::new(selected_rows));
            table.on_header_click = Some(Rc::new(move |index| {
                if let Some(column) = display_columns.get(index) {
                    sort_controller.sort_by(*column);
                }
            }));
        });
        Self { inner }
    }

    pub fn customize(mut self, customize: impl FnOnce(&mut crate::RawTable)) -> Self {
        self.inner = self.inner.customize(customize);
        self
    }
}

impl Widget for RawDataTable {
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

impl Styled for RawDataTable {
    fn set_style(&mut self, style: creamui_core::Style) {
        self.inner.set_style(style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtering_sorting_selection_and_column_order_use_original_indices() {
        let controller = DataTableController::new(2);
        let rows = vec![
            vec!["B".into(), "10".into()],
            vec!["A".into(), "2".into()],
            vec!["A".into(), "5".into()],
        ];
        controller.sort_by(1);
        assert_eq!(controller.visible_rows(&rows), vec![1, 2, 0]);
        controller.sort_by(1);
        assert_eq!(controller.visible_rows(&rows), vec![0, 2, 1]);
        controller.set_filter("A");
        assert_eq!(controller.visible_rows(&rows), vec![2, 1]);
        controller.toggle_row(2);
        assert_eq!(controller.selected(), vec![2]);
        assert!(controller.reorder_column(0, 1));
        assert_eq!(controller.visible_columns(), vec![1, 0]);
        controller.set_column_visible(0, false);
        assert_eq!(controller.visible_columns(), vec![1]);
    }

    #[test]
    fn virtual_data_table_mounts_header_and_filtered_rows() {
        let controller = DataTableController::new(2);
        controller.set_filter("ready");
        let state = VirtualListState::new(2, 32.0);
        let mut table = RawDataTable::new(
            Style::default(),
            ScrollController::new(0.0),
            vec![
                TableColumn::new("Name", 80.0),
                TableColumn::new("State", 80.0),
            ],
            vec![
                vec!["Alpha".into(), "ready".into()],
                vec!["Beta".into(), "waiting".into()],
            ],
            &controller,
            state.clone(),
            100.0,
        );
        assert_eq!(state.len(), 1);
        let mut children = table.children();
        let mut header = children.remove(0);
        let cells = header.children();
        cells[0].on_click().unwrap()();
        assert_eq!(controller.sort(), Some((0, SortDirection::Ascending)));
    }
}
