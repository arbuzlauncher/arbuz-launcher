fn main() {
    tauri_build::build();
    #[cfg(windows)]
    copy_python_runtime();
}

/// PyO3 links against the stable-ABI `python3.dll`, so Windows resolves it
/// before `main()` runs. Copy the embeddable runtime next to the built binary
/// so `target/{debug,release}/arbuz-launcher.exe` starts without the installer.
#[cfg(windows)]
fn copy_python_runtime() {
    use std::path::PathBuf;

    let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let Ok(out_dir) = std::env::var("OUT_DIR") else {
        return;
    };

    // OUT_DIR = target/<profile>/build/<crate>-<hash>/out
    let out_path = PathBuf::from(&out_dir);
    let Some(profile_dir) = out_path.ancestors().nth(3).map(|p| p.to_path_buf()) else {
        return;
    };

    let src = PathBuf::from(manifest).join("python-runtime");
    for name in ["python3.dll", "python312.dll", "python312.zip"] {
        let from = src.join(name);
        if !from.is_file() {
            continue;
        }
        let to = profile_dir.join(name);
        if let Err(e) = std::fs::copy(&from, &to) {
            println!("cargo:warning=could not copy {name}: {e}");
        }
    }
    println!("cargo:rerun-if-changed=python-runtime");
}
