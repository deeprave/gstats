//! Application orchestration module

pub mod initialization;
pub mod execution;

pub use initialization::{
    load_configuration, 
    configure_logging
};
pub use execution::{
    handle_plugin_commands,
    handle_show_branch_command,
    run_scanner
};