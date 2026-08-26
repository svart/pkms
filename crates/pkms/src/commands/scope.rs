use pkms_org::{DailyNoteScope, ScopeFilter};

use crate::{cli::ScopeArgs, config::ResolvedConfig};

pub fn filter_from_args(args: &ScopeArgs, config: &ResolvedConfig) -> ScopeFilter {
    ScopeFilter {
        db_root: config.resolved_db_root().to_path_buf(),
        include_tags: args.include_tags.clone(),
        exclude_tags: args.exclude_tags.clone(),
        path_prefix: args.path_prefix.clone(),
        daily_notes: if args.with_dailies {
            DailyNoteScope::Include
        } else if args.without_dailies {
            DailyNoteScope::Exclude
        } else {
            DailyNoteScope::CommandDefault
        },
        modified_since: args.modified_since,
    }
}
