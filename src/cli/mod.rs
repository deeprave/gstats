//! CLI module containing argument parsing and related functionality

pub mod args;
pub mod enhanced_parser;
pub mod date_parser;
pub mod converter;
pub mod memory_parser;
pub mod plugin_handler;
pub mod command_mapper;
pub mod suggestion;
pub mod contextual_help;
pub mod help_formatter;
pub mod global_flags;

#[cfg(test)]
pub mod tests;

pub use args::Args;
pub use global_flags::filter_global_flags;
