use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use cairo_lang_filesystem::ids::FileId;
use cairo_lang_parser::utils::{get_syntax_file_and_diagnostics, SimpleParserDatabase};

use crate::plugin::DojoPlugin;

pub fn build_corelib(paths: Vec<PathBuf>) {
    let db = &mut SimpleParserDatabase::default();
    for filepath in paths {
        println!("Building corelib for : {}", filepath.display());
        let file_id = FileId::new(db, filepath.clone());
        let mut file = File::open(filepath).unwrap();
        // update file contents by appending node
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        let (syntax_file, _diagnostics) =
            get_syntax_file_and_diagnostics(db, file_id, contents.as_str());
        let plugin = DojoPlugin {};
        for item in syntax_file.items(db).elements(db).into_iter() {
            plugin.generate_corelib(db, item.clone());
        }
    }

    println!("done")
}

pub fn reset_corelib(path: PathBuf) {
    println!("Corelib path: {}", path.display());

    fs::copy(path.join("bases/starknet.cairo"), path.join("starknet.cairo")).unwrap();
    fs::copy(path.join("bases/dojo.cairo"), path.join("dojo.cairo")).unwrap();
    fs::copy(path.join("bases/serde.cairo"), path.join("serde.cairo")).unwrap();
    fs::copy(path.join("bases/lib.cairo"), path.join("lib.cairo")).unwrap();
}
