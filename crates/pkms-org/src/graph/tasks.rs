#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskStateConfig {
    pub valid_states: Vec<String>,
    pub open_states: Vec<String>,
    pub closed_states: Vec<String>,
}
