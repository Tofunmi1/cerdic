//! Content stamps for successful full-gate stages.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::Failure;

/// Stage durations and cache status reported at the end of `check`.
#[derive(Default)]
pub(crate) struct Report(BTreeMap<String, (String, Duration)>);

impl Report {
    pub(crate) fn record(&mut self, name: &str, status: &str, duration: Duration) {
        self.0
            .insert(name.to_owned(), (status.to_owned(), duration));
    }

    pub(crate) fn print(&self) {
        println!("\nGate stages:");
        for (name, (status, duration)) in &self.0 {
            println!("  {name:<16} {status:<8} {:>5.1}s", duration.as_secs_f64());
        }
        let cached = self
            .0
            .iter()
            .filter_map(|(name, (status, _))| (status == "cached").then_some(name.as_str()))
            .collect::<Vec<_>>();
        println!(
            "Cached stages: {}",
            if cached.is_empty() {
                "none".to_owned()
            } else {
                cached.join(", ")
            }
        );
    }
}

/// Hashes selected working-tree files and relevant compiler/runtime version strings with git.
pub(crate) fn key(paths: &[&str]) -> Result<String, Failure> {
    let mut qualified = paths
        .iter()
        .map(|path| {
            path.strip_prefix("../")
                .map_or_else(|| format!("engine/{path}"), str::to_owned)
        })
        .collect::<Vec<_>>();
    if !qualified.iter().any(|path| path.ends_with("xtask")) {
        qualified.push("engine/xtask".to_owned());
    }
    let paths = qualified.iter().map(String::as_str).collect::<Vec<_>>();
    key_at(Path::new(".."), &paths)
}

fn key_at(root: &Path, paths: &[&str]) -> Result<String, Failure> {
    let mut input = file_records(root, paths)?;
    input.extend_from_slice(&tool_records());
    let hash = git_with_input(root, &["hash-object", "--stdin"], &input)?;
    Ok(String::from_utf8_lossy(&hash).trim().to_owned())
}

fn file_records(root: &Path, paths: &[&str]) -> Result<Vec<u8>, Failure> {
    let mut all_paths = paths.to_vec();
    if !all_paths.iter().any(|path| path.ends_with("xtask")) {
        all_paths.push("xtask");
    }
    let mut args = vec![
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
    ];
    args.extend(all_paths);
    let output = git_at(root, &args)?;
    let mut files = output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    let mut input = Vec::new();
    let mut existing = Vec::new();
    for path in &files {
        input.extend_from_slice(path.as_bytes());
        input.push(0);
        if root.join(path).is_file() {
            existing.push(path.as_str());
        }
    }
    let batch_hashes = if existing.iter().any(|path| path.contains('\n')) {
        existing
            .iter()
            .map(|path| {
                git_at(root, &["hash-object", "--", path]).map(|hash| hash.trim_ascii().to_vec())
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let names = existing.join("\n");
        git_with_input(root, &["hash-object", "--stdin-paths"], names.as_bytes())?
            .split(|byte| *byte == b'\n')
            .filter(|hash| !hash.is_empty())
            .map(<[u8]>::to_vec)
            .collect()
    };
    let mut hash_index = 0;
    for path in files {
        if root.join(&path).is_file() {
            input.extend_from_slice(
                batch_hashes
                    .get(hash_index)
                    .ok_or("git omitted a content hash")?,
            );
            hash_index += 1;
        } else {
            input.extend_from_slice(b"deleted");
        }
        input.push(0);
    }
    Ok(input)
}

fn tool_records() -> Vec<u8> {
    let mut input = Vec::new();
    for (program, arg) in [
        ("rustc", "-Vv"),
        ("cargo", "-V"),
        ("bun", "--version"),
        ("node", "--version"),
        ("clang-format", "--version"),
        ("clang++", "--version"),
        ("cmake", "--version"),
        ("ninja", "--version"),
        ("cargo-nextest", "--version"),
        ("cargo-deny", "--version"),
    ] {
        input.extend_from_slice(program.as_bytes());
        input.push(0);
        input.extend_from_slice(&version(program, arg));
        input.push(0);
    }
    for name in [
        "WASI_SDK",
        "VE_WEB_JOBS",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
    ] {
        input.extend_from_slice(name.as_bytes());
        input.push(0);
        input.extend_from_slice(std::env::var(name).unwrap_or_default().as_bytes());
        input.push(0);
    }
    if let Some(sdk) = crate::cpp::wasi_sdk() {
        let tidy = sdk.join("bin/clang-tidy");
        input.extend_from_slice(sdk.to_string_lossy().as_bytes());
        input.extend_from_slice(&version_path(&tidy, "--version"));
    }
    input
}

fn version(program: &str, arg: &str) -> Vec<u8> {
    Command::new(program)
        .arg(arg)
        .output()
        .ok()
        .filter(|result| result.status.success())
        .map_or_else(|| b"unavailable".to_vec(), |result| result.stdout)
}

fn version_path(program: &Path, arg: &str) -> Vec<u8> {
    Command::new(program)
        .arg(arg)
        .output()
        .ok()
        .filter(|result| result.status.success())
        .map_or_else(|| b"unavailable".to_vec(), |result| result.stdout)
}

fn git_at(root: &Path, args: &[&str]) -> Result<Vec<u8>, Failure> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!("git {} failed", args.join(" ")).into())
    }
}

fn git_with_input(root: &Path, args: &[&str], input: &[u8]) -> Result<Vec<u8>, Failure> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("git stdin unavailable")?
        .write_all(input)?;
    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!("git {} failed", args.join(" ")).into())
    }
}

fn stamp_path(name: &str) -> std::path::PathBuf {
    Path::new("target/gate-stamps").join(name)
}

pub(crate) fn matches(name: &str, key: &str) -> bool {
    fs::read_to_string(stamp_path(name)).is_ok_and(|stored| stored.lines().next() == Some(key))
}

pub(crate) fn cached_at(name: &str) -> String {
    fs::read_to_string(stamp_path(name))
        .ok()
        .and_then(|stored| stored.lines().nth(1).map(str::to_owned))
        .unwrap_or_else(|| "unknown time".to_owned())
}

pub(crate) fn write(name: &str, key: &str) -> Result<(), Failure> {
    let path = stamp_path(name);
    fs::create_dir_all(path.parent().ok_or("stamp path has no parent")?)?;
    let timestamp = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map_or_else(
            || "unknown time".to_owned(),
            |output| String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        );
    fs::write(path, format!("{key}\n{timestamp}\n"))?;
    Ok(())
}

pub(crate) fn clear(name: &str) -> Result<(), Failure> {
    match fs::remove_file(stamp_path(name)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn run_fresh(
    name: &str,
    key: &str,
    action: impl FnOnce() -> Result<(), Failure>,
) -> Result<(), Failure> {
    clear(name)?;
    action()?;
    write(name, key)
}

#[cfg(test)]
mod tests {
    use super::{clear, key, key_at, matches, run_fresh, write};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn repo() -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("xtask-stamp-{}-{unique}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"]);
        root
    }

    fn git(root: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }

    #[test]
    fn keys_are_stable_and_change_with_contents() {
        let root = repo();
        fs::write(root.join("input.txt"), "one").unwrap();
        let first = key_at(&root, &["input.txt"]).unwrap();
        assert_eq!(first, key_at(&root, &["input.txt"]).unwrap());
        fs::write(root.join("input.txt"), "two").unwrap();
        assert_ne!(first, key_at(&root, &["input.txt"]).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn paths_can_cross_from_engine_to_workspace_root() {
        let digest = key(&["../package.json", "crates/ve-geometry"]).unwrap();
        assert_eq!(digest.len(), 40);
    }

    #[test]
    fn untracked_files_are_part_of_the_key() {
        let root = repo();
        fs::write(root.join("input.txt"), "tracked").unwrap();
        git(&root, &["add", "input.txt"]);
        let first = key_at(&root, &["."]).unwrap();
        fs::write(root.join("new.txt"), "untracked").unwrap();
        assert_ne!(first, key_at(&root, &["."]).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_stage_run_does_not_leave_a_passing_stamp() {
        let name = format!("failed-test-{}", std::process::id());
        write(&name, "old-pass").unwrap();
        let failed = run_fresh(&name, "new-inputs", || Err("stage failed".into()));
        assert!(failed.is_err());
        assert!(!matches(&name, "old-pass"));
        assert!(!matches(&name, "new-inputs"));
        clear(&name).unwrap();
    }

    #[test]
    fn browser_scoped_key_ignores_docs_but_tracks_page_changes() {
        let root = repo();
        fs::create_dir_all(root.join("web")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("web/index.html"), "page one").unwrap();
        fs::write(root.join("docs/note.md"), "note one").unwrap();
        let initial = key_at(&root, &["web"]).unwrap();
        fs::write(root.join("docs/note.md"), "note two").unwrap();
        assert_eq!(initial, key_at(&root, &["web"]).unwrap());
        fs::write(root.join("web/index.html"), "page two").unwrap();
        assert_ne!(initial, key_at(&root, &["web"]).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
