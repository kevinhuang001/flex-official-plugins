use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
pub const CANCELLED: &str = "已取消";
pub fn check_cancelled(c: &AtomicBool) -> Result<(), String> {
    if c.load(Ordering::Relaxed) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}
pub fn copy_stream(
    input: &mut impl Read,
    output: &mut impl Write,
    c: &AtomicBool,
) -> Result<u64, String> {
    let mut data = [0; 262144];
    let mut total = 0;
    loop {
        check_cancelled(c)?;
        let n = input.read(&mut data).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(total);
        }
        output.write_all(&data[..n]).map_err(|e| e.to_string())?;
        total += n as u64;
    }
}
fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}
pub fn unique(path: &Path) -> PathBuf {
    if !exists(path) {
        return path.into();
    }
    let stem = if path.is_dir() {
        path.file_name()
    } else {
        path.file_stem()
    }
    .unwrap_or_default()
    .to_string_lossy();
    let ext = (!path.is_dir())
        .then(|| path.extension())
        .flatten()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for i in 2.. {
        let next = path.with_file_name(format!("{stem} ({i}){ext}"));
        if !exists(&next) {
            return next;
        }
    }
    unreachable!()
}
