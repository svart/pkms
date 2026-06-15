use crate::cli::OutputFormat;
use anyhow::Result;
use serde::Serialize;
use std::str::FromStr;

pub mod table;
pub mod terminal_markup;
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

impl FromStr for Column {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "id" => Ok(Column::Id),
            "date" => Ok(Column::Date),
            "state" => Ok(Column::State),
            "type" => Ok(Column::Type),
            "prio" => Ok(Column::Prio),
            "tags" => Ok(Column::Tags),
            "project" => Ok(Column::Project),
            "note" => Ok(Column::Note),
            "heading" => Ok(Column::Heading),
            _ => Err(()),
        }
    }
}

impl Column {
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
        assert_eq!("id".parse::<Column>(), Ok(Column::Id));
        assert_eq!("DATE".parse::<Column>(), Ok(Column::Date));
        assert_eq!("Tags".parse::<Column>(), Ok(Column::Tags));
        assert_eq!("project".parse::<Column>(), Ok(Column::Project));
        assert_eq!("heading".parse::<Column>(), Ok(Column::Heading));
    }

    #[test]
    fn test_column_from_str_invalid() {
        assert!("invalid".parse::<Column>().is_err());
        assert!("".parse::<Column>().is_err());
    }

    #[test]
    fn test_column_name() {
        assert_eq!(Column::Id.name(), "Id");
        assert_eq!(Column::Date.name(), "Date");
        assert_eq!(Column::Project.name(), "Project");
        assert_eq!(Column::Heading.name(), "Heading");
    }
}
