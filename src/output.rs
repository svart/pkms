use crate::cli::OutputFormat;
use anyhow::Result;
use serde::Serialize;

pub struct OutputContext {
    pub format: OutputFormat,
    pub no_header: bool,
    pub count_only: bool,
}

impl OutputContext {
    pub fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json | OutputFormat::Ndjson)
    }

    #[allow(clippy::unused_self)]
    pub fn print_json<T: Serialize>(&self, output: &T) -> Result<()> {
        println!("{}", serde_json::to_string_pretty(output)?);
        Ok(())
    }

    #[allow(clippy::unused_self)]
    pub fn print_ndjson<T: Serialize>(&self, items: &[T]) -> Result<()> {
        for item in items {
            println!("{}", serde_json::to_string(item)?);
        }
        Ok(())
    }

    pub fn print_count(&self, count: usize) {
        if self.is_json() {
            println!("{}", serde_json::json!({"count": count}));
        } else {
            println!("{count}");
        }
    }
}
