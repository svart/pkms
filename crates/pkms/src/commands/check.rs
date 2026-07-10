use crate::cli::CheckArgs;
use crate::command_context::CommandContext;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_db::commands::check::{
    self, CheckConfig, CheckItem, CheckOptions, CheckOutput, CheckSelection, CrossLinkTargets,
};
use pkms_db::link_check::SshFileCheckOptions;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub fn run(ctx: &CommandContext<'_>, opts: &CheckOptions) -> Result<ExitCode> {
    let org_config = ctx.config().org_config();
    let output = check::execute(
        &CheckConfig {
            org: org_config,
            ssh: ssh_file_check_options_from_env(),
        },
        opts,
    )?;
    render(ctx.output(), &output)
}

pub fn render(ctx: &OutputContext, output: &CheckOutput) -> Result<ExitCode> {
    if ctx.is_structured() {
        ctx.print_structured(output)?;
    } else {
        print!("{}", check::render_text(output));
    }

    Ok(if output.healthy {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
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

fn ssh_file_check_options_from_env() -> SshFileCheckOptions {
    let mut options = SshFileCheckOptions {
        default_user: default_ssh_user(),
        agent_socket: non_empty_env_os("SSH_AUTH_SOCK").map(PathBuf::from),
        ..SshFileCheckOptions::default()
    };
    if let Some(home) = dirs::home_dir() {
        options.known_hosts = home.join(".ssh").join("known_hosts");
        options.identity_files = default_identity_files(&home);
    }
    options
}

fn default_ssh_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default()
}

fn default_identity_files(home: &Path) -> Vec<PathBuf> {
    ["id_ed25519", "id_ecdsa", "id_rsa"]
        .into_iter()
        .map(|name| home.join(".ssh").join(name))
        .filter(|path| path.is_file())
        .collect()
}

fn non_empty_env_os(key: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(key).filter(|value| !value.as_os_str().is_empty())
}
