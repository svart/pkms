use crate::config::ResolvedConfig;
use crate::output::OutputContext;
use anyhow::Result;
use pkms_org::{Graph, Workspace};

pub struct CommandContext<'a> {
    config: &'a ResolvedConfig,
    output: &'a OutputContext,
}

impl<'a> CommandContext<'a> {
    pub fn new(config: &'a ResolvedConfig, output: &'a OutputContext) -> Self {
        Self { config, output }
    }

    pub fn config(&self) -> &'a ResolvedConfig {
        self.config
    }

    pub fn output(&self) -> &'a OutputContext {
        self.output
    }

    pub fn load_graph(&self) -> Result<Graph> {
        Graph::load(&self.config.org_config())
    }

    pub fn load_workspace(&self) -> Result<Workspace> {
        Workspace::load(&self.config.org_config())
    }
}
