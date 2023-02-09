use std::path::{Path, PathBuf};

pub fn get_cairo_files_in_path(dir: &Path) -> Vec<PathBuf> {
    let mut cairo_files: Vec<PathBuf> = vec![];
    let dir_iter = dir.read_dir().unwrap();
    for dir_entry in dir_iter {
        let path = dir_entry.unwrap().path();
        if path.is_dir() {
            cairo_files.append(&mut get_cairo_files_in_path(&path));
        } else {
            let extn = path.extension();
            if extn.is_some() && extn.unwrap().to_str().unwrap() == "cairo" {
                cairo_files.push(path);
            }
        }
    }
    cairo_files
}

#[test]
fn test_get_cairo_files_in_path() {
    let mut test_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    test_path.pop();
    test_path.push("lib");

    let files = get_cairo_files_in_path(&test_path);
    assert!(!files.is_empty());
}
