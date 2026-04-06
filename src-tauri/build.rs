use std::path::PathBuf;

fn main() {
    tauri_build::build();

    // Copy espeak-ng-data to the target directory so espeak-rs can find it at runtime.
    // espeak-rs looks for espeak-ng-data next to the executable.
    let espeak_data_src = find_espeak_data();
    if let Some(src) = espeak_data_src {
        let target_dir = get_target_dir();
        let dst = target_dir.join("espeak-ng-data");
        if !dst.exists() || dir_is_older(&dst, &src) {
            println!("cargo:warning=Copying espeak-ng-data to {}", dst.display());
            let _ = std::fs::remove_dir_all(&dst);
            copy_dir_recursive(&src, &dst).expect("failed to copy espeak-ng-data");
        }
    }
}

fn find_espeak_data() -> Option<PathBuf> {
    // Walk up from the target dir to find the espeak-rs-sys build output
    let target_dir = get_target_dir();
    let build_dir = target_dir.join("build");
    if build_dir.exists() {
        for entry in std::fs::read_dir(&build_dir).ok()? {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("espeak-rs-sys-") {
                let share = entry.path().join("out/share/espeak-ng-data");
                if share.exists() {
                    return Some(share);
                }
            }
        }
    }
    None
}

fn get_target_dir() -> PathBuf {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let profile = std::env::var("PROFILE").unwrap();
    let mut dir = out_dir.as_path();
    while let Some(parent) = dir.parent() {
        if parent.ends_with(&profile) {
            return parent.to_path_buf();
        }
        dir = parent;
    }
    out_dir
}

fn dir_is_older(dst: &PathBuf, src: &PathBuf) -> bool {
    let dst_time = std::fs::metadata(dst)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    let src_time = std::fs::metadata(src)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    dst_time < src_time
}

fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}
