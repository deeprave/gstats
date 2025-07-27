use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let version_api_path = Path::new(&out_dir).join("version_api.rs");
    
    // Read API version from Cargo.toml metadata
    let cargo_manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let cargo_toml_path = Path::new(&cargo_manifest).join("Cargo.toml");
    let cargo_toml_content = fs::read_to_string(&cargo_toml_path)
        .expect("Failed to read Cargo.toml");
    
    // Parse TOML to extract api_version
    let cargo_toml: toml::Value = cargo_toml_content.parse()
        .expect("Failed to parse Cargo.toml");
    
    let api_version = cargo_toml
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("gstats"))
        .and_then(|g| g.get("api_version"))
        .and_then(|v| v.as_integer())
        .expect("Failed to find package.metadata.gstats.api_version in Cargo.toml");
    
    let version_content = format!(
        "// Auto-generated API version from Cargo.toml metadata\n\
         // Source: package.metadata.gstats.api_version = {}\n\
         // This version is controlled in Cargo.toml and committed to source control\n\
         pub const BASE_API_VERSION: i64 = {};\n",
        api_version, api_version
    );
    
    fs::write(&version_api_path, version_content)
        .expect("Failed to write version_api.rs");
    
    println!("cargo:warning=Generated API version {} from Cargo.toml", api_version);
    
    // Tell cargo to rerun if Cargo.toml changes
    println!("cargo:rerun-if-changed=Cargo.toml");
}
