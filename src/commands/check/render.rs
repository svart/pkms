use super::model::{CheckCommandOutput, CheckOutput};
use crate::output::OutputContext;
use anyhow::Result;
use std::fmt::Write;
use std::process::ExitCode;

pub(super) fn render(ctx: &OutputContext, output: &CheckCommandOutput) -> Result<ExitCode> {
    if ctx.is_structured() {
        ctx.print_structured(&output.output)?;
    } else {
        print!("{}", render_text(&output.output));
    }

    Ok(output.exit_code)
}

pub fn render_text(output: &CheckOutput) -> String {
    let mut text = String::new();
    let has_sections = output.has_sections();

    if has_sections {
        render_summary(&mut text, output);
    }

    render_duplicate_sections(&mut text, output);
    render_broken_links_section(&mut text, output);
    render_broken_file_links_section(&mut text, output);
    render_file_link_errors_section(&mut text, output);
    render_broken_attachment_links_section(&mut text, output);
    render_filetags_section(&mut text, output);
    render_self_links_section(&mut text, output);
    render_overlinks_section(&mut text, output);
    render_cross_links_section(&mut text, output);

    if has_sections {
        text.push('\n');
    }
    render_status(&mut text, output.healthy);

    text
}

fn render_summary(text: &mut String, output: &CheckOutput) {
    let _ = writeln!(text, "Database: {}", output.db_root);

    if let Some(stats) = &output.stats {
        let _ = writeln!(text, "  Notes:          {}", stats.total_notes);
        let _ = writeln!(text, "  Links:          {}", stats.total_links);
        let _ = writeln!(text, "    internal: {}", stats.total_internal_links);
        let _ = writeln!(text, "    url: {}", stats.total_url_links);
        let _ = writeln!(text, "    file: {}", stats.total_file_links);
        #[cfg(feature = "ssh")]
        let _ = writeln!(text, "    ssh: {}", stats.total_ssh_links);
        let _ = writeln!(text, "  Orphans:        {}", stats.orphan_notes);
        let _ = writeln!(text, "  Broken links:   {}", stats.broken_link_count);
        let _ = writeln!(text, "  Parse errors:   {}", stats.parse_error_count);
        let _ = writeln!(text, "  Skipped files:  {}", stats.skipped_count);
        let _ = writeln!(text, "  Dup UUIDs:      {}", stats.duplicate_uuid_count);
        let _ = writeln!(text, "  Dup titles:     {}", stats.duplicate_title_count);
        let _ = writeln!(text, "  Missing titles: {}", stats.missing_title_count);
    }

    if let Some(broken_file) = &output.broken_file_links {
        let _ = writeln!(text, "  Broken files:   {}", broken_file.len());
    }
    if let Some(file_link_errors) = &output.file_link_errors {
        let _ = writeln!(text, "  File errors:    {}", file_link_errors.len());
    }
    if let Some(broken_attachment) = &output.broken_attachment_links {
        let _ = writeln!(text, "  Broken attach:  {}", broken_attachment.len());
    }
    if let Some(filetags_issues) = &output.filetags_issues {
        let _ = writeln!(text, "  Filetags issues: {}", filetags_issues.len());
    }
    if let Some(overlinks) = &output.overlinks {
        let _ = writeln!(text, "  Overlinks:      {}", overlinks.len());
    }
}

fn render_duplicate_sections(text: &mut String, output: &CheckOutput) {
    if let Some(duplicates) = &output.duplicates {
        if !duplicates.duplicate_uuids.is_empty() {
            text.push('\n');
            let _ = writeln!(
                text,
                "Duplicate UUIDs ({}):",
                duplicates.duplicate_uuids.len()
            );
            for d in &duplicates.duplicate_uuids {
                for p in &d.paths {
                    let _ = writeln!(text, "  {} -> {}", d.value, p);
                }
            }
        }

        if !duplicates.duplicate_titles.is_empty() {
            text.push('\n');
            let _ = writeln!(
                text,
                "Duplicate titles ({}):",
                duplicates.duplicate_titles.len()
            );
            for d in &duplicates.duplicate_titles {
                for p in &d.paths {
                    let _ = writeln!(text, "  \"{}\" -> {}", d.value, p);
                }
            }
        }

        if !duplicates.missing_titles.is_empty() {
            text.push('\n');
            let _ = writeln!(
                text,
                "Missing #+title ({}):",
                duplicates.missing_titles.len()
            );
            for p in &duplicates.missing_titles {
                let _ = writeln!(text, "  {p}");
            }
        }
    }
}

fn render_broken_links_section(text: &mut String, output: &CheckOutput) {
    if let Some(broken_links) = &output.broken_links
        && !broken_links.is_empty()
    {
        text.push('\n');
        let _ = writeln!(text, "Broken links ({}):", broken_links.len());
        for entry in broken_links {
            let title = if entry.source_title.is_empty() {
                "?"
            } else {
                entry.source_title.as_str()
            };
            let _ = writeln!(text, "  {title} -> {}", entry.target_uuid);
        }
    }
}

fn render_broken_file_links_section(text: &mut String, output: &CheckOutput) {
    if let Some(broken_file) = &output.broken_file_links
        && !broken_file.is_empty()
    {
        text.push('\n');
        let _ = writeln!(text, "Broken file links ({}):", broken_file.len());
        for entry in broken_file {
            let _ = writeln!(text, "  {} -> {}", entry.source_title, entry.target_path);
        }
    }
}

fn render_file_link_errors_section(text: &mut String, output: &CheckOutput) {
    if let Some(file_link_errors) = &output.file_link_errors
        && !file_link_errors.is_empty()
    {
        text.push('\n');
        let _ = writeln!(text, "File link errors ({}):", file_link_errors.len());
        for entry in file_link_errors {
            let _ = writeln!(
                text,
                "  {} -> {} [{}:{}] {}",
                entry.source_title,
                entry.target_path,
                entry.backend,
                entry.error_kind,
                entry.message
            );
        }
    }
}

fn render_broken_attachment_links_section(text: &mut String, output: &CheckOutput) {
    if let Some(broken_attachment) = &output.broken_attachment_links
        && !broken_attachment.is_empty()
    {
        text.push('\n');
        let _ = writeln!(
            text,
            "Broken attachment links ({}):",
            broken_attachment.len()
        );
        for entry in broken_attachment {
            let _ = writeln!(text, "  {} -> {}", entry.source_title, entry.target_path);
        }
    }
}

fn render_filetags_section(text: &mut String, output: &CheckOutput) {
    if let Some(filetags_issues) = &output.filetags_issues
        && !filetags_issues.is_empty()
    {
        text.push('\n');
        let _ = writeln!(text, "Invalid filetags format ({}):", filetags_issues.len());
        for entry in filetags_issues {
            let _ = writeln!(text, "  {} ({}): {}", entry.title, entry.path, entry.issue);
        }
    }
}

fn render_self_links_section(text: &mut String, output: &CheckOutput) {
    if let Some(self_links) = &output.self_links
        && !self_links.is_empty()
    {
        text.push('\n');
        let _ = writeln!(text, "Self-referencing links ({}):", self_links.len());
        for entry in self_links {
            match entry.suggestion.as_ref() {
                Some(suggestion) => {
                    let _ = writeln!(
                        text,
                        "  {} — {} link to self: {} ({})",
                        entry.source_title, entry.link_type, entry.target, suggestion
                    );
                }
                None => {
                    let _ = writeln!(
                        text,
                        "  {} — {} link to self: {}",
                        entry.source_title, entry.link_type, entry.target
                    );
                }
            }
        }
    }
}

fn render_overlinks_section(text: &mut String, output: &CheckOutput) {
    if let Some(overlinks) = &output.overlinks
        && !overlinks.is_empty()
    {
        text.push('\n');
        let _ = writeln!(
            text,
            "Overlinking (2+ links to the same note) ({}):",
            overlinks.len()
        );
        for entry in overlinks {
            let _ = writeln!(
                text,
                "  \"{}\" -> \"{}\" ({}x)",
                entry.source_title, entry.target_title, entry.count
            );
        }
    }
}

fn render_cross_links_section(text: &mut String, output: &CheckOutput) {
    if let Some(cr) = &output.cross_links {
        text.push('\n');
        let _ = writeln!(
            text,
            "Cross-links between \"{}\" and \"{}\":",
            cr.source_title, cr.target_title
        );
        let _ = writeln!(
            text,
            "  \"{}\" -> \"{}\": {} link(s)",
            cr.source_title, cr.target_title, cr.source_to_target
        );
        let _ = writeln!(
            text,
            "  \"{}\" -> \"{}\": {} link(s)",
            cr.target_title, cr.source_title, cr.target_to_source
        );
    }
}

fn render_status(text: &mut String, healthy: bool) {
    if healthy {
        text.push_str("Status: healthy\n");
    } else {
        text.push_str("Status: issues found\n");
    }
}

impl CheckOutput {
    fn has_sections(&self) -> bool {
        self.stats.is_some()
            || self.broken_file_links.is_some()
            || self.file_link_errors.is_some()
            || self.broken_attachment_links.is_some()
            || self.filetags_issues.is_some()
            || self.duplicates.is_some()
            || self.broken_links.is_some()
            || self.failed_files.is_some()
            || self.self_links.is_some()
            || self.overlinks.is_some()
            || self.cross_links.is_some()
    }
}
