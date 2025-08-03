//! Plugin System Tests
//! 
//! Comprehensive tests for the plugin system with mock implementations.

pub mod mock_plugins;

#[cfg(test)]
pub mod registry_tests;

#[cfg(test)]
pub mod version_tests;

#[cfg(test)]
pub mod notification_tests;

#[cfg(test)]
pub mod discovery_tests;