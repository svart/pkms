use super::Column;

pub fn terminal_width() -> Option<usize> {
    let columns = std::env::var("COLUMNS").ok();
    let detected = terminal_size::terminal_size().map(|(w, _)| w.0 as usize);
    terminal_width_from(columns.as_deref(), detected)
}

fn terminal_width_from(columns: Option<&str>, detected: Option<usize>) -> Option<usize> {
    columns
        .and_then(|s| s.parse().ok())
        .filter(|&w| w > 0)
        .or(detected)
}

const MIN_COLUMN_WIDTH: usize = 15;
const TAG_WEIGHT: f64 = 0.20;
const NOTE_WEIGHT: f64 = 0.35;
const HEADING_WEIGHT: f64 = 0.45;

#[derive(Clone, Copy)]
struct ColumnWidths<'a> {
    widths: &'a [usize],
}

impl<'a> ColumnWidths<'a> {
    fn new(widths: &'a [usize]) -> Self {
        Self { widths }
    }

    fn for_column(self, column: Column) -> usize {
        self.widths[column.index()]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TableLayout {
    column_widths: Vec<(Column, usize)>,
}

impl TableLayout {
    fn new(column_widths: Vec<(Column, usize)>) -> Self {
        Self { column_widths }
    }

    fn from_max_widths(enabled_columns: &[Column], max_widths: ColumnWidths<'_>) -> Self {
        Self::new(
            enabled_columns
                .iter()
                .map(|c| (*c, max_widths.for_column(*c)))
                .collect(),
        )
    }

    pub(crate) fn column_widths(&self) -> &[(Column, usize)] {
        &self.column_widths
    }

    pub(crate) fn rendered_width(&self) -> usize {
        self.column_widths.iter().map(|(_, w)| *w).sum::<usize>()
            + table_padding_width(self.column_widths.len())
    }
}

pub(crate) fn table_padding_width(n_columns: usize) -> usize {
    n_columns.saturating_sub(1) + 2 * n_columns
}

fn is_fixed_column(col: Column) -> bool {
    matches!(
        col,
        Column::Id | Column::Date | Column::State | Column::Type | Column::Prio
    )
}

fn is_wrap_column(col: Column) -> bool {
    matches!(
        col,
        Column::Tags | Column::Project | Column::Note | Column::Heading
    )
}

fn wrap_weight(col: Column) -> f64 {
    match col {
        Column::Tags => TAG_WEIGHT,
        Column::Project => NOTE_WEIGHT,
        Column::Note => NOTE_WEIGHT,
        Column::Heading => HEADING_WEIGHT,
        _ => 0.0,
    }
}

pub(crate) fn adaptive_table_layout(
    enabled_columns: &[Column],
    max_widths: &[usize],
) -> Option<TableLayout> {
    let term_w = terminal_width()?;
    compute_layout(enabled_columns, max_widths, term_w)
}

fn compute_layout(
    enabled_columns: &[Column],
    max_widths: &[usize],
    term_w: usize,
) -> Option<TableLayout> {
    let max_widths = ColumnWidths::new(max_widths);
    let budget = TableLayoutBudget::new(enabled_columns, max_widths, term_w);
    let wrap_cols = wrap_columns(enabled_columns);

    if wrap_cols.is_empty() {
        return Some(TableLayout::from_max_widths(enabled_columns, max_widths));
    }

    if budget.available_for_wrapped_columns < wrap_cols.len() * MIN_COLUMN_WIDTH {
        return None;
    }

    let wrap_widths =
        compute_wrap_widths(&wrap_cols, max_widths, budget.available_for_wrapped_columns)?;
    let layout = project_table_layout(enabled_columns, max_widths, &wrap_widths);

    if layout
        .column_widths()
        .iter()
        .any(|(c, w)| is_wrap_column(*c) && *w < MIN_COLUMN_WIDTH)
    {
        return None;
    }

    Some(layout)
}

struct TableLayoutBudget {
    available_for_wrapped_columns: usize,
}

impl TableLayoutBudget {
    fn new(enabled_columns: &[Column], max_widths: ColumnWidths<'_>, term_w: usize) -> Self {
        let fixed_width: usize = enabled_columns
            .iter()
            .filter(|c| is_fixed_column(**c))
            .map(|c| max_widths.for_column(*c))
            .sum();
        let padding = table_padding_width(enabled_columns.len());

        Self {
            available_for_wrapped_columns: term_w.saturating_sub(fixed_width + padding),
        }
    }
}

fn wrap_columns(enabled_columns: &[Column]) -> Vec<Column> {
    enabled_columns
        .iter()
        .copied()
        .filter(|c| is_wrap_column(*c))
        .collect()
}

fn compute_wrap_widths(
    wrap_cols: &[Column],
    max_widths: ColumnWidths<'_>,
    available: usize,
) -> Option<Vec<(Column, usize)>> {
    let mut widths = allocate_wrap_widths(wrap_cols, available);
    cap_and_redistribute_wrap_widths(&mut widths, max_widths, available);
    enforce_minimum_wrap_widths(&mut widths, available)?;
    Some(widths)
}

fn allocate_wrap_widths(wrap_cols: &[Column], available: usize) -> Vec<(Column, usize)> {
    let total_weight: f64 = wrap_cols.iter().map(|c| wrap_weight(*c)).sum();

    let mut widths: Vec<(Column, usize)> = wrap_cols
        .iter()
        .map(|&col| {
            let w = (available as f64 * wrap_weight(col) / total_weight).floor() as usize;
            (col, w)
        })
        .collect();

    let sum_now: usize = widths.iter().map(|(_, w)| w).sum();
    if sum_now < available {
        let mut rem = available - sum_now;
        widths.sort_by(|a, b| wrap_weight(b.0).partial_cmp(&wrap_weight(a.0)).unwrap());
        for (_, w) in &mut widths {
            if rem == 0 {
                break;
            }
            *w += 1;
            rem -= 1;
        }
        widths.sort_by_key(|&(col, _)| wrap_cols.iter().position(|&c| c == col).unwrap());
    }

    widths
}

fn cap_and_redistribute_wrap_widths(
    widths: &mut [(Column, usize)],
    max_widths: ColumnWidths<'_>,
    available: usize,
) {
    loop {
        if !cap_wrap_widths_to_content(widths, max_widths) {
            break;
        }

        let capped_sum: usize = widths.iter().map(|(_, w)| w).sum();
        let leftover = available.saturating_sub(capped_sum);
        if leftover == 0 {
            break;
        }

        if !redistribute_leftover_to_uncapped_wrap_widths(widths, max_widths, leftover) {
            break;
        }
    }
}

fn cap_wrap_widths_to_content(
    widths: &mut [(Column, usize)],
    max_widths: ColumnWidths<'_>,
) -> bool {
    let mut any_capped = false;

    for (col, w) in widths.iter_mut() {
        let max_cw = max_widths.for_column(*col);
        if *w > max_cw {
            *w = max_cw;
            any_capped = true;
        }
    }

    any_capped
}

fn redistribute_leftover_to_uncapped_wrap_widths(
    widths: &mut [(Column, usize)],
    max_widths: ColumnWidths<'_>,
    leftover: usize,
) -> bool {
    let uncapped: Vec<usize> = widths
        .iter()
        .enumerate()
        .filter(|(_, (col, w))| *w < max_widths.for_column(*col))
        .map(|(i, _)| i)
        .collect();
    let uncapped_weight: f64 = uncapped.iter().map(|&i| wrap_weight(widths[i].0)).sum();
    if uncapped_weight == 0.0 {
        return false;
    }

    let mut added_sum = 0usize;
    for &i in &uncapped {
        let add = (leftover as f64 * wrap_weight(widths[i].0) / uncapped_weight).floor() as usize;
        widths[i].1 += add;
        added_sum += add;
    }

    let mut rem = leftover.saturating_sub(added_sum);
    for &i in uncapped.iter().rev() {
        if rem == 0 {
            break;
        }
        widths[i].1 += 1;
        rem -= 1;
    }

    true
}

fn enforce_minimum_wrap_widths(widths: &mut [(Column, usize)], available: usize) -> Option<()> {
    for (_, w) in widths.iter_mut() {
        if *w < MIN_COLUMN_WIDTH {
            *w = MIN_COLUMN_WIDTH;
        }
    }
    let sum_after_min: usize = widths.iter().map(|(_, w)| w).sum();
    if sum_after_min > available {
        let mut excess = sum_after_min - available;
        widths.sort_by(|a, b| wrap_weight(a.0).partial_cmp(&wrap_weight(b.0)).unwrap());
        for (_, w) in widths.iter_mut() {
            if excess == 0 {
                break;
            }
            let can_take = (*w).saturating_sub(MIN_COLUMN_WIDTH);
            let take = can_take.min(excess);
            *w -= take;
            excess -= take;
        }
        if excess > 0 {
            return None;
        }
    }

    Some(())
}

fn project_table_layout(
    enabled_columns: &[Column],
    max_widths: ColumnWidths<'_>,
    wrap_widths: &[(Column, usize)],
) -> TableLayout {
    let mut column_widths: Vec<(Column, usize)> = Vec::new();
    for &col in enabled_columns {
        if is_fixed_column(col) {
            column_widths.push((col, max_widths.for_column(col)));
        } else if let Some(&(_, w)) = wrap_widths.iter().find(|&&(c, _)| c == col) {
            column_widths.push((col, w));
        }
    }

    TableLayout::new(column_widths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width_for(widths: &[(Column, usize)], column: Column) -> usize {
        widths
            .iter()
            .find_map(|(col, width)| (*col == column).then_some(*width))
            .unwrap()
    }

    #[test]
    fn test_terminal_width_prefers_columns_env() {
        assert_eq!(terminal_width_from(Some("120"), Some(80)), Some(120));
    }

    #[test]
    fn test_terminal_width_uses_detected_width_when_columns_invalid() {
        assert_eq!(terminal_width_from(Some("0"), Some(80)), Some(80));
        assert_eq!(terminal_width_from(Some("wide"), Some(80)), Some(80));
    }

    #[test]
    fn test_adaptive_column_widths_only_fixed() {
        let cols = [Column::Id, Column::State];
        let max_widths = [5, 0, 10, 0, 0, 0, 0, 0, 0];
        let result = compute_layout(&cols, &max_widths, 120);
        assert!(result.is_some());
        let widths = result.unwrap().column_widths().to_vec();
        assert_eq!(widths.len(), 2);
        assert_eq!(widths[0], (Column::Id, 5));
        assert_eq!(widths[1], (Column::State, 10));
    }

    #[test]
    fn test_adaptive_column_widths_term_too_small() {
        let cols = [Column::Id, Column::State, Column::Tags, Column::Heading];
        let max_widths = [5, 0, 10, 0, 0, 30, 0, 0, 50];
        let result = compute_layout(&cols, &max_widths, 20);
        assert!(result.is_none());
    }

    #[test]
    fn test_adaptive_column_widths_wide_terminal() {
        let cols = [
            Column::Id,
            Column::State,
            Column::Prio,
            Column::Note,
            Column::Heading,
        ];
        let max_widths = [5, 0, 10, 0, 6, 0, 0, 60, 60];
        let result = compute_layout(&cols, &max_widths, 200);
        assert!(result.is_some());
        let layout = result.unwrap();
        assert!(layout.column_widths().len() >= 5);
    }

    #[test]
    fn test_adaptive_column_widths_full_todo_agenda_layout_fits_terminal() {
        let cols = [
            Column::Id,
            Column::Date,
            Column::State,
            Column::Type,
            Column::Prio,
            Column::Tags,
            Column::Note,
            Column::Heading,
        ];
        let max_widths = [3, 10, 5, 5, 4, 36, 0, 42, 70];
        let layout = compute_layout(&cols, &max_widths, 120).unwrap();
        let column_widths = ColumnWidths::new(&max_widths);
        let widths = layout.column_widths();

        assert_eq!(widths.iter().map(|(col, _)| *col).collect::<Vec<_>>(), cols);
        assert!(layout.rendered_width() <= 120);
        for (column, width) in widths {
            assert!(
                *width <= column_widths.for_column(*column),
                "{column:?} exceeded its content width"
            );
            if is_wrap_column(*column) {
                assert!(*width >= MIN_COLUMN_WIDTH, "{column:?} is too narrow");
            }
        }
    }

    #[test]
    fn test_adaptive_column_widths_respects_user_selected_column_order() {
        let cols = [Column::Heading, Column::Id, Column::Note];
        let max_widths = [3, 0, 0, 0, 0, 0, 0, 50, 80];
        let layout = compute_layout(&cols, &max_widths, 100).unwrap();
        let widths = layout.column_widths();

        assert_eq!(
            widths.iter().map(|(col, _)| *col).collect::<Vec<_>>(),
            vec![Column::Heading, Column::Id, Column::Note]
        );
        assert!(layout.rendered_width() <= 100);
    }

    #[test]
    fn test_adaptive_column_widths_weights_heading_more_than_note_and_tags() {
        let cols = [Column::Tags, Column::Note, Column::Heading];
        let max_widths = [0, 0, 0, 0, 0, 100, 0, 100, 100];
        let layout = compute_layout(&cols, &max_widths, 108).unwrap();
        let widths = layout.column_widths();

        assert!(width_for(widths, Column::Tags) < width_for(widths, Column::Note));
        assert!(width_for(widths, Column::Note) < width_for(widths, Column::Heading));
        assert_eq!(layout.rendered_width(), 108);
    }

    #[test]
    fn test_adaptive_column_widths_caps_short_content_columns() {
        let cols = [Column::Tags, Column::Note, Column::Heading];
        let max_widths = [0, 0, 0, 0, 0, 18, 0, 80, 80];
        let layout = compute_layout(&cols, &max_widths, 120).unwrap();
        let widths = layout.column_widths();

        assert_eq!(width_for(widths, Column::Tags), 18);
        assert!(width_for(widths, Column::Note) <= 80);
        assert!(width_for(widths, Column::Heading) <= 80);
        assert!(layout.rendered_width() <= 120);
    }

    #[test]
    fn test_adaptive_column_widths_uses_minimum_for_each_wrapped_column() {
        let cols = [Column::Tags, Column::Note, Column::Heading];
        let max_widths = [0, 0, 0, 0, 0, 100, 0, 100, 100];
        let layout = compute_layout(&cols, &max_widths, 53).unwrap();
        let widths = layout.column_widths();

        assert_eq!(width_for(widths, Column::Tags), MIN_COLUMN_WIDTH);
        assert_eq!(width_for(widths, Column::Note), MIN_COLUMN_WIDTH);
        assert_eq!(width_for(widths, Column::Heading), MIN_COLUMN_WIDTH);
        assert_eq!(layout.rendered_width(), 53);
    }
}
