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

#[cfg(test)]
pub mod integration_tests; // Phase 5: End-to-end integration tests

#[cfg(test)]
pub mod priority_tests;

#[cfg(test)]
pub mod activation_tests;

#[cfg(test)]
pub mod builtin_initialization_tests;

#[cfg(test)]
pub mod coordination_tests;

#[cfg(test)]
pub mod encapsulation_tests;

