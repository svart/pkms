use anyhow::Result;
use std::path::Path;

pub fn read_lines(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    let mut lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
    if content.is_empty() || !content.ends_with('\n') {
        let consumed: usize = lines.iter().map(String::len).sum();
        if consumed < content.len() {
            lines.push(content[consumed..].to_string());
        }
    }
    Ok(lines)
}

pub fn write_lines(path: impl AsRef<Path>, lines: &[String]) -> Result<()> {
    std::fs::write(path, lines.concat())?;
    Ok(())
}

pub fn split_line_ending(line: &str) -> (&str, &'static str) {
    let newline = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    (line.strip_suffix(newline).unwrap_or(line), newline)
}
