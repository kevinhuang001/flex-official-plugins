use serde::Serialize;
use std::{collections::HashMap, path::Path, process::Command};

#[derive(Serialize)]
pub struct GitStatus {
    pub root: String,
    pub branch: String,
    pub entries: HashMap<String, String>,
}

fn output(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Returns branch and per-file status when `git` is installed and `path` is in a
/// work tree. Status keys are absolute canonical paths so they match listings.
pub fn status(path: &str) -> Option<GitStatus> {
    if let Some(route) = crate::git_history::route(path) {
        return Some(GitStatus {
            root: route.root.to_string_lossy().into_owned(),
            branch: route.commit[..8].into(),
            entries: HashMap::new(),
        });
    }
    if which::which("git").is_err() {
        return None;
    }
    let root = output(Path::new(path), &["rev-parse", "--show-toplevel"])?;
    let root = dunce::canonicalize(root).ok()?;
    let mut branch =
        output(&root, &["symbolic-ref", "--short", "HEAD"]).unwrap_or_else(|| "HEAD".into());
    if branch == "HEAD" {
        branch = output(&root, &["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "HEAD".into());
    }
    let raw = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .output()
        .ok()?;
    if !raw.status.success() {
        return None;
    }
    let mut entries = HashMap::new();
    let mut fields = raw.stdout.split(|byte| *byte == 0);
    while let Some(field) = fields.next() {
        if field.len() < 4 {
            continue;
        }
        let xy = &field[..2];
        let relative = String::from_utf8_lossy(&field[3..]).replace('\\', "/");
        let code = String::from_utf8_lossy(xy).into_owned();
        let absolute = root.join(relative).to_string_lossy().into_owned();
        entries.insert(absolute, code);

        if xy.contains(&b'R') || xy.contains(&b'C') {
            // Rename/copy records are followed by the original path.
            let _ = fields.next();
        }
    }
    Some(GitStatus {
        root: root.to_string_lossy().into_owned(),
        branch,
        entries,
    })
}

#[derive(Serialize)]
pub struct GitRefs {
    pub head: String,
    pub current: String,
    pub branches: Vec<String>,
    pub tags: Vec<String>,
}

pub fn refs(root: &str) -> Result<GitRefs, String> {
    let current = output(Path::new(root), &["symbolic-ref", "--short", "HEAD"])
        .unwrap_or_else(|| "HEAD".into());
    let branches = output(
        Path::new(root),
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .unwrap_or_default()
    .lines()
    .map(str::to_string)
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>();
    let tags = output(
        Path::new(root),
        &["for-each-ref", "--format=%(refname:short)", "refs/tags"],
    )
    .unwrap_or_default()
    .lines()
    .map(str::to_string)
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>();
    Ok(GitRefs {
        head: output(Path::new(root), &["rev-parse", "--short", "HEAD"]).unwrap_or_default(),
        current,
        branches,
        tags,
    })
}

pub(crate) fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("--literal-pathspecs")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .output()
        .map_err(|error| format!("无法运行 Git，请确认已安装 Git：{error}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().into())
    } else {
        let message = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if message.is_empty() {
            "Git 操作未完成".into()
        } else {
            message
        })
    }
}

pub fn init(path: &str) -> Result<GitStatus, String> {
    let path = dunce::canonicalize(path).map_err(|error| error.to_string())?;
    if !path.is_dir() {
        return Err("请选择本地文件夹来创建仓库".into());
    }
    if let Some(existing) = status(&path.to_string_lossy()) {
        return Ok(existing);
    }
    run(&path, &["init", "--initial-branch=main"])?;
    status(&path.to_string_lossy()).ok_or_else(|| "仓库已创建，但无法读取状态".into())
}

pub(crate) fn repository(root: &str) -> Result<std::path::PathBuf, String> {
    let root = dunce::canonicalize(root).map_err(|error| error.to_string())?;
    let found = run(&root, &["rev-parse", "--show-toplevel"])?;
    dunce::canonicalize(found).map_err(|error| error.to_string())
}

/// Stage explicit, literal repository-relative paths; never interpret filename globs as pathspecs.
pub fn stage(root: &str, paths: &[String], all: bool) -> Result<(), String> {
    let root = repository(root)?;
    if all {
        run(&root, &["add", "--all", "--", "."])?;
        return Ok(());
    }
    let paths = relative_paths(&root, paths)?;
    if paths.is_empty() {
        return Err("请选择要暂存的文件".into());
    }
    let mut args = vec!["add", "--all", "--"];
    args.extend(paths.iter().map(String::as_str));
    run(&root, &args)?;
    Ok(())
}

fn relative_paths(root: &Path, paths: &[String]) -> Result<Vec<String>, String> {
    paths
        .iter()
        .map(|value| {
            let path = Path::new(value);
            if !path.is_absolute()
                || path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err("文件路径无效".into());
            }
            // Resolve parent aliases (/var -> /private/var on macOS), while
            // preserving the final filename and paths of deleted tracked files.
            let mut ancestor = path.parent().ok_or("文件路径无效")?;
            let mut tail = vec![path.file_name().ok_or("文件路径无效")?.to_os_string()];
            while !ancestor.exists() {
                tail.push(ancestor.file_name().ok_or("文件路径无效")?.to_os_string());
                ancestor = ancestor.parent().ok_or("文件路径无效")?;
            }
            let mut resolved = dunce::canonicalize(ancestor).map_err(|e| e.to_string())?;
            for part in tail.into_iter().rev() {
                resolved.push(part);
            }
            let relative = resolved
                .strip_prefix(root)
                .map_err(|_| "文件不在当前仓库中")?;
            if relative.as_os_str().is_empty() {
                return Err("文件路径无效".into());
            }
            Ok(relative.to_string_lossy().into_owned())
        })
        .collect()
}

pub fn unstage(root: &str, paths: &[String], all: bool) -> Result<(), String> {
    let root = repository(root)?;
    let paths = relative_paths(&root, paths)?;
    if !all && paths.is_empty() {
        return Err("请选择要取消暂存的文件".into());
    }
    let has_head = run(&root, &["rev-parse", "--verify", "HEAD"]).is_ok();
    let mut args = if has_head {
        vec!["reset", "--quiet", "HEAD", "--"]
    } else {
        vec!["rm", "--cached", "-r", "--ignore-unmatch", "--"]
    };
    if all {
        args.push(".");
    } else {
        args.extend(paths.iter().map(String::as_str));
    }
    run(&root, &args)?;
    Ok(())
}

#[derive(Serialize)]
pub struct GitIdentity {
    pub name: String,
    pub email: String,
}
pub fn identity(root: &str) -> GitIdentity {
    let root = Path::new(root);
    GitIdentity {
        name: output(root, &["config", "user.name"]).unwrap_or_default(),
        email: output(root, &["config", "user.email"]).unwrap_or_default(),
    }
}

pub fn commit(root: &str, message: &str, name: &str, email: &str) -> Result<String, String> {
    let root = repository(root)?;
    let message = message.trim();
    if message.is_empty() {
        return Err("请填写提交说明".into());
    }
    if name.trim().is_empty() || email.trim().is_empty() {
        return Err("请填写提交者姓名和邮箱".into());
    }
    run(&root, &["config", "--local", "user.name", name.trim()])?;
    run(&root, &["config", "--local", "user.email", email.trim()])?;
    run(&root, &["commit", "-m", message])?;
    run(&root, &["rev-parse", "--short", "HEAD"])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }
    #[test]
    fn initialize_stage_unstage_first_commit_and_modify() {
        let temp = setup();
        let root = temp.path().to_str().unwrap();
        std::fs::write(temp.path().join("notes.txt"), "first").unwrap();
        let initialized = init(root).unwrap();
        assert_eq!(initialized.branch, "main");
        stage(root, &[], true).unwrap();
        unstage(root, &[], true).unwrap();
        assert!(temp.path().join("notes.txt").exists());
        assert!(status(root).unwrap().entries.values().any(|v| v == "??"));
        stage(root, &[], true).unwrap();
        assert!(
            !commit(root, "First commit", "Fiex Test", "fiex@example.com")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            output(temp.path(), &["config", "--local", "user.name"]).unwrap(),
            "Fiex Test"
        );
        assert_eq!(
            output(temp.path(), &["config", "--local", "user.email"]).unwrap(),
            "fiex@example.com"
        );
        assert!(status(root).unwrap().entries.is_empty());
        std::fs::write(temp.path().join("notes.txt"), "second").unwrap();
        stage(root, &[], true).unwrap();
        unstage(root, &[], true).unwrap();
        assert!(status(root).unwrap().entries.values().any(|v| v == " M"));
    }
    #[test]
    fn paths_are_literal_and_cannot_escape_repository() {
        let temp = setup();
        let root = temp.path().to_str().unwrap();
        init(root).unwrap();
        for name in ["[draft].txt", "draft.txt", "-option.txt"] {
            std::fs::write(temp.path().join(name), "x").unwrap();
        }
        stage(
            root,
            &[temp
                .path()
                .join("[draft].txt")
                .to_string_lossy()
                .into_owned()],
            false,
        )
        .unwrap();
        let state = status(root).unwrap();
        let canonical = dunce::canonicalize(temp.path()).unwrap();
        assert_eq!(
            state.entries[&canonical.join("[draft].txt").to_string_lossy().into_owned()],
            "A "
        );
        assert_eq!(
            state.entries[&canonical.join("draft.txt").to_string_lossy().into_owned()],
            "??"
        );
        assert!(stage(
            root,
            &[temp
                .path()
                .join("../outside")
                .to_string_lossy()
                .into_owned()],
            false
        )
        .is_err());
        assert!(commit(root, " ", "Test", "test@example.com").is_err());
    }
}
