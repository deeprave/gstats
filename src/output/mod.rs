//! Output formatting and display module

pub mod reports;
pub mod authors;

pub use reports::{
    format_compact_table,
    display_plugin_reports,
    display_plugin_response,
    display_plugin_data,
    create_file_statistics_summary,
    execute_plugins_analysis
};
pub use authors::{
    display_author_report,
    display_author_insights
};