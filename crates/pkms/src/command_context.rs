use crate::config::ResolvedConfig;
use crate::output::OutputContext;

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
}
