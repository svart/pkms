use crate::cli::OutputFormat;
use anyhow::Result;
use serde::Serialize;

pub mod table;
pub use table::adaptive_column_widths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Id,
    Date,
    State,
    Type,
    Prio,
    Tags,
    Project,
    Note,
    Heading,
}

pub const ALL_COLUMNS: &[Column; 9] = &[
    Column::Id,
    Column::Date,
    Column::State,
    Column::Type,
    Column::Prio,
    Column::Tags,
    Column::Project,
    Column::Note,
    Column::Heading,
];

impl Column {
    pub fn from_str(s: &str) -> Option<Column> {
        match s.to_lowercase().as_str() {
            "id" => Some(Column::Id),
            "date" => Some(Column::Date),
            "state" => Some(Column::State),
            "type" => Some(Column::Type),
            "prio" => Some(Column::Prio),
            "tags" => Some(Column::Tags),
            "project" => Some(Column::Project),
            "note" => Some(Column::Note),
            "heading" => Some(Column::Heading),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Column::Id => "Id",
            Column::Date => "Date",
            Column::State => "State",
            Column::Type => "Type",
            Column::Prio => "Prio",
            Column::Tags => "Tags",
            Column::Project => "Project",
            Column::Note => "Note",
            Column::Heading => "Heading",
        }
    }
}

pub struct OutputContext {
    pub format: OutputFormat,
}

impl OutputContext {
    pub fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json | OutputFormat::Ndjson)
    }

    pub fn print_json<T: Serialize + ?Sized>(&self, output: &T) -> Result<()> {
        let _ = self;
        println!("{}", serde_json::to_string_pretty(output)?);
        Ok(())
    }

    pub fn print_ndjson<T: Serialize>(&self, items: &[T]) -> Result<()> {
        let _ = self;
        for item in items {
            println!("{}", serde_json::to_string(item)?);
        }
        Ok(())
    }

    pub fn print_json_adaptive<T: Serialize>(&self, items: &[T]) -> Result<()> {
        if items.len() == 1 {
            self.print_json(&items[0])
        } else {
            self.print_json(items)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_from_str_valid() {
        assert_eq!(Column::from_str("id"), Some(Column::Id));
        assert_eq!(Column::from_str("DATE"), Some(Column::Date));
        assert_eq!(Column::from_str("Tags"), Some(Column::Tags));
        assert_eq!(Column::from_str("project"), Some(Column::Project));
        assert_eq!(Column::from_str("heading"), Some(Column::Heading));
    }

    #[test]
    fn test_column_from_str_invalid() {
        assert_eq!(Column::from_str("invalid"), None);
        assert_eq!(Column::from_str(""), None);
    }

    #[test]
    fn test_column_name() {
        assert_eq!(Column::Id.name(), "Id");
        assert_eq!(Column::Date.name(), "Date");
        assert_eq!(Column::Project.name(), "Project");
        assert_eq!(Column::Heading.name(), "Heading");
    }
}
