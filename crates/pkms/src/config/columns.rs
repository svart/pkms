use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ColumnsConfig {
    Global(Vec<String>),
    Matrix(ColumnMatrixConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ColumnMatrixConfig {
    pub pkms: Option<SourceColumnConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceColumnConfig {
    pub tasks: Option<Vec<String>>,
    pub agenda: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnSource {
    Pkms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnView {
    Tasks,
    Agenda,
}

impl ColumnsConfig {
    pub fn default_for(&self, source: ColumnSource, view: ColumnView) -> Result<Option<&[String]>> {
        match self {
            ColumnsConfig::Global(columns) => Ok(Some(columns.as_slice())),
            ColumnsConfig::Matrix(matrix) => matrix.default_for(source, view),
        }
    }
}

impl ColumnMatrixConfig {
    fn default_for(&self, source: ColumnSource, view: ColumnView) -> Result<Option<&[String]>> {
        match source {
            ColumnSource::Pkms => Ok(source_default(self.pkms.as_ref(), view)),
        }
    }
}

fn source_default(source: Option<&SourceColumnConfig>, view: ColumnView) -> Option<&[String]> {
    source.and_then(|source| match view {
        ColumnView::Tasks => source.tasks.as_deref(),
        ColumnView::Agenda => source.agenda.as_deref(),
    })
}

pub(super) fn default_columns_for(
    columns: Option<&ColumnsConfig>,
    source: ColumnSource,
    view: ColumnView,
) -> Result<Option<&[String]>> {
    columns
        .map(|columns| columns.default_for(source, view))
        .transpose()
        .map(Option::flatten)
}
