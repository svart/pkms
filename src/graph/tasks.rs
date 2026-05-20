use super::Graph;
use crate::config::ResolvedConfig;
use crate::util::priority_value;

impl Graph {
    pub fn all_task_entries(&self, config: &ResolvedConfig) -> Vec<(usize, String, usize)> {
        let entries = self.sorted_task_entries(config);
        entries
            .into_iter()
            .enumerate()
            .map(|(i, (p, l, _, _))| (i + 1, p, l))
            .collect()
    }

    pub fn resolve_canonical_task_id(
        &self,
        config: &ResolvedConfig,
        id: usize,
    ) -> anyhow::Result<(String, usize)> {
        let entries = self.sorted_task_entries(config);
        if id == 0 || id > entries.len() {
            anyhow::bail!(
                "No task with canonical ID {}. Valid range is 1–{}",
                id,
                entries.len()
            );
        }
        let (path, line, _, _) = &entries[id - 1];
        Ok((path.clone(), *line))
    }

    fn sorted_task_entries(
        &self,
        config: &ResolvedConfig,
    ) -> Vec<(String, usize, Option<char>, bool)> {
        let valid_states = config.todo_states();
        let open_states = config.open_todo_states();
        let closed_states = config.closed_todo_states();
        let mut items: Vec<(String, usize, Option<char>, bool)> = Vec::new();

        for result in &self.results {
            if result.parse_error.is_some() {
                continue;
            }
            for heading in &result.parsed.headings {
                let is_todo = heading
                    .todo_state
                    .as_ref()
                    .is_some_and(|s| valid_states.iter().any(|vs| vs.eq_ignore_ascii_case(s)));
                let has_dates = heading.scheduled.is_some() || heading.deadline.is_some();
                if !is_todo && !has_dates {
                    continue;
                }
                let is_open = match &heading.todo_state {
                    Some(s) if open_states.iter().any(|os| os.eq_ignore_ascii_case(s)) => true,
                    Some(s) if closed_states.iter().any(|cs| cs.eq_ignore_ascii_case(s)) => false,
                    _ => true,
                };
                items.push((
                    result.path.display().to_string(),
                    heading.line_number,
                    heading.priority,
                    is_open,
                ));
            }
        }

        items.sort_by(|a, b| {
            b.3.cmp(&a.3)
                .then_with(|| {
                    let a_p = a.2.map(priority_value).unwrap_or(3);
                    let b_p = b.2.map(priority_value).unwrap_or(3);
                    a_p.cmp(&b_p)
                })
                .then(a.0.cmp(&b.0))
                .then(a.1.cmp(&b.1))
        });

        items
    }
}

#[cfg(test)]
mod tests {
    use crate::util::priority_value;

    #[test]
    fn test_canonical_priority_ordering() {
        assert!(priority_value('A') < priority_value('B'));
        assert!(priority_value('B') < priority_value('C'));
        assert!(priority_value('C') < priority_value('D'));
    }

    #[test]
    fn test_canonical_priority_values() {
        assert_eq!(priority_value('A'), 0);
        assert_eq!(priority_value('B'), 1);
        assert_eq!(priority_value('C'), 2);
        assert_eq!(priority_value('Z'), 3);
        assert_eq!(priority_value('X'), 3);
    }
}
