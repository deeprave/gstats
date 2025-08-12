//! Application orchestration module

pub mod initialization;
pub mod execution;
pub mod repository;

pub use repository::{resolve_repository_path, validate_repository_path};
pub use initialization::{
    load_configuration, 
    configure_logging, 
    create_colour_manager,
    initialize_builtin_plugins,
    create_plugin_context
};
pub use execution::{
    handle_plugin_commands,
    run_scanner
};