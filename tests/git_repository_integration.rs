//! Integration tests for Git Repository handle functionality
//! Tests the refactored git module with Repository handle support

use gstats::git::{validate_git_repository, resolve_repository_path};
use git2::Repository;
use std::path::Path;
use tempfile::TempDir;

/// Helper function to create a test git repository
fn create_test_repo() -> (TempDir, String) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().to_string_lossy().to_string();
    
    Repository::init(&repo_path).expect("Failed to init test repository");
    
    (temp_dir, repo_path)
}

/// Helper function to create a bare git repository
fn create_bare_repo() -> (TempDir, String) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().to_string_lossy().to_string();
    
    Repository::init_bare(&repo_path).expect("Failed to init bare repository");
    
    (temp_dir, repo_path)
}

#[test]
fn test_validate_git_repository_handle_returns_repository() {
    let (_temp_dir, repo_path) = create_test_repo();
    
    // This should fail initially as the function doesn't exist yet
    let result = gstats::git::validate_git_repository_handle(&repo_path);
    assert!(result.is_ok());
    
    let repo = result.unwrap();
    assert!(repo.workdir().unwrap().to_string_lossy().contains(&repo_path.trim_end_matches('/')));
}

#[test]
fn test_validate_git_repository_handle_with_bare_repo() {
    let (_temp_dir, repo_path) = create_bare_repo();
    
    let result = gstats::git::validate_git_repository_handle(&repo_path);
    assert!(result.is_ok());
    
    let repo = result.unwrap();
    assert!(repo.is_bare());
}

#[test]
fn test_validate_git_repository_handle_invalid_path() {
    let result = gstats::git::validate_git_repository_handle("/definitely/not/a/repo");
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.err().unwrap());
    assert!(err_msg.contains("Path does not exist"));
}

#[test]
fn test_validate_git_repository_handle_not_a_repo() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let result = gstats::git::validate_git_repository_handle(temp_dir.path());
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.err().unwrap());
    assert!(err_msg.contains("Failed to open repository"));
}

#[test]
fn test_repository_handle_wrapper_functionality() {
    let (_temp_dir, repo_path) = create_test_repo();
    
    // Test RepositoryHandle wrapper
    let result = gstats::git::RepositoryHandle::open(&repo_path);
    assert!(result.is_ok());
    
    let handle = result.unwrap();
    assert!(handle.path().contains(&repo_path.trim_end_matches('/')));
    assert!(handle.workdir().is_some());
    assert!(!handle.is_bare());
}

#[test]
fn test_repository_handle_with_bare_repository() {
    let (_temp_dir, repo_path) = create_bare_repo();
    
    let result = gstats::git::RepositoryHandle::open(&repo_path);
    assert!(result.is_ok());
    
    let handle = result.unwrap();
    assert!(handle.is_bare());
    assert_eq!(handle.workdir(), None);
}

#[test]
fn test_resolve_repository_handle_with_path() {
    let (_temp_dir, repo_path) = create_test_repo();
    
    let result = gstats::git::resolve_repository_handle(Some(repo_path.clone()));
    assert!(result.is_ok());
    
    let handle = result.unwrap();
    assert!(handle.path().contains(&repo_path.trim_end_matches('/')));
}

#[test]
fn test_resolve_repository_handle_current_directory() {
    // This test assumes we're running from within the gstats git repository
    let result = gstats::git::resolve_repository_handle(None);
    assert!(result.is_ok());
    
    let handle = result.unwrap();
    assert!(handle.path().contains("gstats"));
}

#[test]
fn test_backward_compatibility_validate_git_repository() {
    let (_temp_dir, repo_path) = create_test_repo();
    
    // Original function should still work and return a string path
    let result = validate_git_repository(&repo_path);
    assert!(result.is_ok());
    
    let path_string = result.unwrap();
    assert!(path_string.contains(&repo_path));
}

#[test]
fn test_backward_compatibility_resolve_repository_path() {
    let (_temp_dir, repo_path) = create_test_repo();
    
    // Original function should still work and return a string path
    let result = resolve_repository_path(Some(repo_path.clone()));
    assert!(result.is_ok());
    
    let path_string = result.unwrap();
    assert!(path_string.contains(&repo_path));
}

#[test]
fn test_scanner_init_context_creation() {
    let (_temp_dir, repo_path) = create_test_repo();
    
    // Test ScannerInitContext
    let repo_handle = gstats::git::RepositoryHandle::open(&repo_path).unwrap();
    let config = gstats::scanner::ScannerConfig::default();
    let query = gstats::scanner::QueryParams::default();
    
    let context = gstats::git::ScannerInitContext::new(repo_handle, config, query);
    assert!(context.repository().path().contains(&repo_path.trim_end_matches('/')));
}

#[test]
fn test_repository_handle_thread_safety() {
    let (_temp_dir, repo_path) = create_test_repo();
    let _handle = gstats::git::RepositoryHandle::open(&repo_path).unwrap();
    
    // RepositoryHandle should be Send + Sync for scanner concurrency
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<gstats::git::RepositoryHandle>();
}

#[test]
fn test_repository_handle_conversion() {
    let (_temp_dir, repo_path) = create_test_repo();
    
    // Test conversion between string path and Repository handle
    let handle = gstats::git::RepositoryHandle::open(&repo_path).unwrap();
    let converted_path = handle.to_path_string();
    assert!(converted_path.contains(&repo_path.trim_end_matches('/')));
    
    // Test creating handle from existing Repository
    let repo = Repository::open(&repo_path).unwrap();
    let handle_from_repo = gstats::git::RepositoryHandle::from_repository(repo);
    assert!(handle_from_repo.path().contains(&repo_path.trim_end_matches('/')));
}