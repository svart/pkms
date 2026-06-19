use crate::cli::FixArgs;
use crate::config::ResolvedConfig;
use crate::discovery;
use crate::graph::Graph;
use crate::output::OutputContext;
use anyhow::Result;
use regex::Regex;
use serde::Serialize;

#[derive(Serialize)]
pub struct FixOutput {
    pub broken_uuid: String,
    pub replacement_uuid: String,
    pub replacement_title: String,
    pub files_affected: Vec<String>,
    pub total_replacements: usize,
    pub applied: bool,
}

fn print_fix_output(ctx: &OutputContext, output: &FixOutput) -> Result<()> {
    if ctx.is_structured() {
        ctx.print_structured(output)?;
    } else {
        if output.applied {
            println!(
                "Fixed {} broken link(s) in {} file(s):",
                output.total_replacements,
                output.files_affected.len()
            );
        } else {
            println!(
                "Would fix {} broken link(s) in {} file(s):",
                output.total_replacements,
                output.files_affected.len()
            );
            println!("  Broken UUID: {}", output.broken_uuid);
            println!(
                "  Replace with: {} ({})",
                output.replacement_title, output.replacement_uuid
            );
            println!("  (use --apply to apply)");
        }
        for f in &output.files_affected {
            println!("  {f}");
        }
    }

    Ok(())
}

fn find_and_replace_links(
    db_root: &std::path::Path,
    ignore_patterns: &[String],
    broken_str: &str,
    replacement_uuid: &str,
    apply: bool,
) -> Result<(Vec<String>, usize)> {
    let mut files_affected = Vec::new();
    let mut total_replacements = 0;

    let escaped = regex::escape(broken_str);
    let re = Regex::new(&format!(r"(id:){escaped}")).unwrap();

    for path in discovery::walk_org_files(db_root, ignore_patterns)? {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        let count = re.find_iter(&content).count();
        if count > 0 {
            files_affected.push(path.display().to_string());
            total_replacements += count;

            if apply {
                let new_content = re.replace_all(&content, |caps: &regex::Captures| {
                    format!("{}{}", &caps[1], replacement_uuid)
                });
                std::fs::write(&path, new_content.as_ref())?;
            }
        }
    }

    Ok((files_affected, total_replacements))
}

pub struct FixOptions {
    pub broken_uuid: String,
    pub target_uuid: String,
    pub apply: bool,
}

impl TryFrom<&FixArgs> for FixOptions {
    type Error = anyhow::Error;

    fn try_from(args: &FixArgs) -> Result<Self> {
        let broken_uuid = uuid::Uuid::parse_str(&args.broken_uuid)
            .map_err(|_| anyhow::anyhow!("Invalid UUID format: {}", args.broken_uuid))?
            .to_string();
        let target_uuid = uuid::Uuid::parse_str(&args.target)
            .map_err(|_| anyhow::anyhow!("Invalid UUID format: {}", args.target))?
            .to_string();
        Ok(FixOptions {
            broken_uuid,
            target_uuid,
            apply: args.apply,
        })
    }
}

pub fn run(config: &ResolvedConfig, ctx: &OutputContext, opts: &FixOptions) -> Result<()> {
    let graph = Graph::load(config)?;
    let db_root = config.resolved_db_root();
    let ignore_patterns = config.resolve_ignore_patterns();

    let (replacement_uuid, replacement_title) = graph
        .nodes
        .get(&opts.target_uuid)
        .map(|n| (n.uuid.clone(), n.title.clone()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Replacement UUID not found in database: {}",
                opts.target_uuid
            )
        })?;

    let (files_affected, total_replacements) = find_and_replace_links(
        db_root,
        &ignore_patterns,
        &opts.broken_uuid,
        &replacement_uuid,
        opts.apply,
    )?;

    let output = FixOutput {
        broken_uuid: opts.broken_uuid.clone(),
        replacement_uuid: replacement_uuid.clone(),
        replacement_title,
        files_affected: files_affected.clone(),
        total_replacements,
        applied: opts.apply,
    };

    print_fix_output(ctx, &output)
}
