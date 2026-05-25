use super::Column;

pub fn terminal_width() -> Option<usize> {
    if let Some((w, _)) = terminal_size::terminal_size() {
        return Some(w.0 as usize);
    }
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&w| w > 0)
}

const MIN_COLUMN_WIDTH: usize = 15;
const TAG_WEIGHT: f64 = 0.20;
const NOTE_WEIGHT: f64 = 0.35;
const HEADING_WEIGHT: f64 = 0.45;

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

pub fn adaptive_column_widths(
    enabled_columns: &[Column],
    max_widths: &[usize],
) -> Option<Vec<(Column, usize)>> {
    let term_w = terminal_width()?;
    compute_widths(enabled_columns, max_widths, term_w)
}

fn compute_widths(
    enabled_columns: &[Column],
    max_widths: &[usize],
    term_w: usize,
) -> Option<Vec<(Column, usize)>> {
    let n_columns = enabled_columns.len();
    let padding = n_columns.saturating_sub(1) + 2 * n_columns;

    let fixed_width: usize = enabled_columns
        .iter()
        .filter(|c| is_fixed_column(**c))
        .map(|c| max_widths[*c as usize])
        .sum();

    let available = term_w.saturating_sub(fixed_width + padding);
    let min_col = MIN_COLUMN_WIDTH;

    let wrap_cols: Vec<Column> = enabled_columns
        .iter()
        .copied()
        .filter(|c| is_wrap_column(*c))
        .collect();

    let n_wrap = wrap_cols.len();

    if n_wrap == 0 {
        return Some(
            enabled_columns
                .iter()
                .map(|c| (*c, max_widths[*c as usize]))
                .collect(),
        );
    }

    if available < n_wrap * min_col {
        return None;
    }

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

    loop {
        let mut any_capped = false;

        for (col, w) in &mut widths {
            let max_cw = max_widths[*col as usize];
            if *w > max_cw {
                *w = max_cw;
                any_capped = true;
            }
        }

        if !any_capped {
            break;
        }

        let capped_sum: usize = widths.iter().map(|(_, w)| w).sum();
        let leftover = available.saturating_sub(capped_sum);
        if leftover == 0 {
            break;
        }

        let uncapped: Vec<usize> = widths
            .iter()
            .enumerate()
            .filter(|(_, (col, w))| *w < max_widths[*col as usize])
            .map(|(i, _)| i)
            .collect();
        let uncapped_weight: f64 = uncapped.iter().map(|&i| wrap_weight(widths[i].0)).sum();
        if uncapped_weight == 0.0 {
            break;
        }

        let mut added_sum = 0usize;
        for &i in &uncapped {
            let add =
                (leftover as f64 * wrap_weight(widths[i].0) / uncapped_weight).floor() as usize;
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
    }

    for (_, w) in &mut widths {
        if *w < min_col {
            *w = min_col;
        }
    }
    let sum_after_min: usize = widths.iter().map(|(_, w)| w).sum();
    if sum_after_min > available {
        let mut excess = sum_after_min - available;
        widths.sort_by(|a, b| wrap_weight(a.0).partial_cmp(&wrap_weight(b.0)).unwrap());
        for (_, w) in &mut widths {
            if excess == 0 {
                break;
            }
            let can_take = (*w).saturating_sub(min_col);
            let take = can_take.min(excess);
            *w -= take;
            excess -= take;
        }
        if excess > 0 {
            return None;
        }
    }

    let mut result: Vec<(Column, usize)> = Vec::new();
    for &col in enabled_columns {
        if is_fixed_column(col) {
            result.push((col, max_widths[col as usize]));
        } else if let Some(&(_, w)) = widths.iter().find(|&&(c, _)| c == col) {
            result.push((col, w));
        }
    }

    let wrap_min_check: Vec<&(Column, usize)> =
        result.iter().filter(|(c, _)| is_wrap_column(*c)).collect();
    if wrap_min_check.iter().any(|(_, w)| *w < min_col) {
        return None;
    }

    Some(result)
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

    fn rendered_width(widths: &[(Column, usize)]) -> usize {
        let padding = widths.len().saturating_sub(1) + 2 * widths.len();
        widths.iter().map(|(_, w)| *w).sum::<usize>() + padding
    }

    #[test]
    fn test_adaptive_column_widths_only_fixed() {
        let cols = [Column::Id, Column::State];
        let max_widths = [5, 0, 10, 0, 0, 0, 0, 0, 0];
        let result = compute_widths(&cols, &max_widths, 120);
        assert!(result.is_some());
        let widths = result.unwrap();
        assert_eq!(widths.len(), 2);
        assert_eq!(widths[0], (Column::Id, 5));
        assert_eq!(widths[1], (Column::State, 10));
    }

    #[test]
    fn test_adaptive_column_widths_term_too_small() {
        let cols = [Column::Id, Column::State, Column::Tags, Column::Heading];
        let max_widths = [5, 0, 10, 0, 0, 30, 0, 0, 50];
        let result = compute_widths(&cols, &max_widths, 20);
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
        let result = compute_widths(&cols, &max_widths, 200);
        assert!(result.is_some());
        let widths = result.unwrap();
        assert!(widths.len() >= 5);
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
        let widths = compute_widths(&cols, &max_widths, 120).unwrap();

        assert_eq!(widths.iter().map(|(col, _)| *col).collect::<Vec<_>>(), cols);
        assert!(rendered_width(&widths) <= 120);
        for (column, width) in &widths {
            assert!(
                *width <= max_widths[*column as usize],
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
        let widths = compute_widths(&cols, &max_widths, 100).unwrap();

        assert_eq!(
            widths.iter().map(|(col, _)| *col).collect::<Vec<_>>(),
            vec![Column::Heading, Column::Id, Column::Note]
        );
        assert!(rendered_width(&widths) <= 100);
    }

    #[test]
    fn test_adaptive_column_widths_weights_heading_more_than_note_and_tags() {
        let cols = [Column::Tags, Column::Note, Column::Heading];
        let max_widths = [0, 0, 0, 0, 0, 100, 0, 100, 100];
        let widths = compute_widths(&cols, &max_widths, 108).unwrap();

        assert!(width_for(&widths, Column::Tags) < width_for(&widths, Column::Note));
        assert!(width_for(&widths, Column::Note) < width_for(&widths, Column::Heading));
        assert_eq!(rendered_width(&widths), 108);
    }

    #[test]
    fn test_adaptive_column_widths_caps_short_content_columns() {
        let cols = [Column::Tags, Column::Note, Column::Heading];
        let max_widths = [0, 0, 0, 0, 0, 18, 0, 80, 80];
        let widths = compute_widths(&cols, &max_widths, 120).unwrap();

        assert_eq!(width_for(&widths, Column::Tags), 18);
        assert!(width_for(&widths, Column::Note) <= 80);
        assert!(width_for(&widths, Column::Heading) <= 80);
        assert!(rendered_width(&widths) <= 120);
    }

    #[test]
    fn test_adaptive_column_widths_uses_minimum_for_each_wrapped_column() {
        let cols = [Column::Tags, Column::Note, Column::Heading];
        let max_widths = [0, 0, 0, 0, 0, 100, 0, 100, 100];
        let widths = compute_widths(&cols, &max_widths, 53).unwrap();

        assert_eq!(width_for(&widths, Column::Tags), MIN_COLUMN_WIDTH);
        assert_eq!(width_for(&widths, Column::Note), MIN_COLUMN_WIDTH);
        assert_eq!(width_for(&widths, Column::Heading), MIN_COLUMN_WIDTH);
        assert_eq!(rendered_width(&widths), 53);
    }
}
