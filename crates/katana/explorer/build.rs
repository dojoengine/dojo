use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=dist");
    
    // Check if the dist directory exists within the explorer crate
    let dist_dir = Path::new("dist");
    if !dist_dir.exists() {
        println!("cargo:warning=Explorer dist directory not found at {:?}. The embedded explorer will not be available.", dist_dir);
        println!("cargo:warning=Please ensure the explorer build files are copied to the 'dist' directory in the explorer crate.");
    } else {
        println!("Using explorer build files from {:?}", dist_dir);
    }
} 