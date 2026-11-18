use std::{env, fs, path::PathBuf, time::SystemTime};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    // classic cargo layout
    println!("cargo:rustc-link-search={}/../../../deps", out.display());
    // since cargo 1.9x the cdylib dependencies end up in build/<crate>/<hash>/out, use the newest
    let build = out.join("../../..");
    for name in ["symmetric_executor", "symmetric_stream"] {
        let Ok(entries) = fs::read_dir(build.join(name)) else {
            continue;
        };
        let newest = entries
            .flatten()
            .map(|e| e.path().join("out"))
            .filter_map(|dir| {
                let modified = fs::metadata(dir.join(format!("lib{name}.so")))
                    .ok()?
                    .modified()
                    .ok()?;
                Some((modified, dir))
            })
            .max_by_key(|(modified, _): &(SystemTime, PathBuf)| *modified);
        if let Some((_, dir)) = newest {
            println!("cargo:rustc-link-search={}", dir.display());
        }
    }
}
