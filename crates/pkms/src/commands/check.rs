use crate::cli::CheckArgs;
use crate::command_context::CommandContext;
use crate::config::SshConfig;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::check::{
    self, CheckCommandOutput, CheckConfig, CheckItem, CheckOptions, CheckSelection,
    CrossLinkTargets,
};
use pkms_db::link_check::SshFileCheckConfig;
use std::process::ExitCode;

pub fn run(ctx: &CommandContext<'_>, opts: &CheckOptions) -> Result<ExitCode> {
    let config = ctx.config().db_command_config();
    let output = check::execute(
        &CheckConfig {
            org: config.org,
            ssh: config.ssh.as_ref().map(ssh_file_check_config_from_config),
        },
        opts,
    )?;
    render(ctx.output(), &output)
}

pub fn render(ctx: &OutputContext, output: &CheckCommandOutput) -> Result<ExitCode> {
    if ctx.is_structured() {
        ctx.print_structured(&output.output)?;
    } else {
        print!("{}", check::render_text(&output.output));
    }

    Ok(output.exit_code)
}

pub fn options_from_args(args: &CheckArgs) -> CheckOptions {
    let mut checks = Vec::new();
    if args.stats {
        checks.push(CheckItem::Stats);
    }
    if args.file_links {
        checks.push(CheckItem::FileLinks);
    }
    if args.remote_file_links {
        checks.push(CheckItem::RemoteFileLinks);
    }
    if args.attachment_links {
        checks.push(CheckItem::AttachmentLinks);
    }
    if args.id_links {
        checks.push(CheckItem::IdLinks);
    }
    if args.filetags {
        checks.push(CheckItem::Filetags);
    }
    if args.self_links {
        checks.push(CheckItem::SelfLinks);
    }
    if args.overlinks {
        checks.push(CheckItem::Overlinks);
    }
    let cross_links = args.cross_links.as_ref().and_then(|targets| {
        let [source, target] = targets.as_slice() else {
            return None;
        };
        Some(CrossLinkTargets {
            source: source.clone(),
            target: target.clone(),
        })
    });
    let checks = if checks.is_empty() && cross_links.is_none() {
        CheckSelection::Default
    } else {
        CheckSelection::Explicit(checks)
    };

    CheckOptions {
        checks,
        cross_links,
    }
}

fn ssh_file_check_config_from_config(config: &SshConfig) -> SshFileCheckConfig {
    SshFileCheckConfig {
        identity_file: config.identity_file.clone(),
        known_hosts: config.known_hosts.clone(),
        connect_timeout_ms: config.connect_timeout_ms,
        operation_timeout_ms: config.operation_timeout_ms,
        max_connections: config.max_connections,
        agent: config.agent,
    }
}
