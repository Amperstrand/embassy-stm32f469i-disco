use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rustc-link-arg-examples=-Tlink.x");
    println!("cargo:rustc-link-arg-examples=-Tdefmt.x");

    if let Err(err) = copy_memory_x_into_cortex_m_rt_out_dirs() {
        panic!("failed to make memory.x available to cortex-m-rt linker output: {err}");
    }
}

fn copy_memory_x_into_cortex_m_rt_out_dirs() -> io::Result<()> {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing"));
    let memory_x = manifest_dir.join("memory.x");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR missing"));
    let target = env::var("TARGET").expect("TARGET missing");

    let Some(target_dir) = find_target_dir(&out_dir, &target) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "could not locate target directory for {target} from OUT_DIR={}",
                out_dir.display()
            ),
        ));
    };

    let mut matched = false;
    copy_into_matching_out_dirs(&memory_x, &target_dir, &mut matched)?;

    if !matched {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no cortex-m-rt output directories found under {}",
                target_dir.display()
            ),
        ));
    }

    Ok(())
}

fn find_target_dir(out_dir: &Path, target: &str) -> Option<PathBuf> {
    out_dir
        .ancestors()
        .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(target))
        .map(Path::to_path_buf)
}

fn copy_into_matching_out_dirs(memory_x: &Path, dir: &Path, matched: &mut bool) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        if is_cortex_m_rt_out_dir(&path) {
            let destination = path.join("memory.x");
            fs::copy(memory_x, &destination)?;
            *matched = true;
            eprintln!("copied {} to {}", memory_x.display(), destination.display());
            continue;
        }

        copy_into_matching_out_dirs(memory_x, &path, matched)?;
    }

    Ok(())
}

fn is_cortex_m_rt_out_dir(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if file_name != "out" {
        return false;
    }

    let Some(parent_name) = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
    else {
        return false;
    };

    parent_name.starts_with("cortex-mrt-") || parent_name.starts_with("cortex-m-rt-")
}
