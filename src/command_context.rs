use crate::config::ResolvedConfig;
use crate::graph::Graph;
use crate::output::OutputContext;
use crate::workspace::Workspace;
use anyhow::Result;

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
        Graph::load(self.config)
    }

    pub fn load_workspace(&self) -> Result<Workspace> {
        Workspace::load(self.config)
    }
}
