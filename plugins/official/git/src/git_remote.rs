use crate::git::{repository, run};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct Remote {
    pub name: String,
    pub url: String,
}
#[derive(Serialize)]
pub struct RemoteStatus {
    pub remotes: Vec<Remote>,
    pub branch: String,
    pub upstream: String,
    pub ahead: usize,
    pub behind: usize,
}

pub fn status(root: &str) -> Result<RemoteStatus, String> {
    let root = repository(root)?;
    let names = run(&root, &["remote"])?;
    let remotes = names
        .lines()
        .map(|name| Remote {
            name: name.into(),
            url: run(&root, &["remote", "get-url", "--", name]).unwrap_or_default(),
        })
        .collect();
    let branch = run(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default();
    let upstream = run(&root, &["rev-parse", "--abbrev-ref", "@{upstream}"]).unwrap_or_default();
    let counts = run(
        &root,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
    )
    .unwrap_or_default();
    let mut counts = counts.split_whitespace().filter_map(|n| n.parse().ok());
    Ok(RemoteStatus {
        remotes,
        branch,
        upstream,
        ahead: counts.next().unwrap_or(0),
        behind: counts.next().unwrap_or(0),
    })
}

pub fn save(root: &str, name: &str, url: &str) -> Result<(), String> {
    if name.is_empty()
        || name.starts_with('-')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    {
        return Err("请输入有效的远程名称".into());
    }
    let url = url.trim();
    if url.is_empty() || url.starts_with('-') || url.contains(['\n', '\r', '\0']) {
        return Err("请输入有效的仓库地址".into());
    }
    let root = repository(root)?;
    let exists = run(&root, &["remote"])?.lines().any(|n| n == name);
    run(
        &root,
        &[
            "remote",
            if exists { "set-url" } else { "add" },
            "--",
            name,
            url,
        ],
    )?;
    Ok(())
}

fn remote<'a>(root: &Path, name: &'a str) -> Result<&'a str, String> {
    if !run(root, &["remote"])?.lines().any(|n| n == name) {
        return Err("请选择已配置的远程仓库".into());
    }
    Ok(name)
}

pub fn remove(root: &str, name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.starts_with('-')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    {
        return Err("请输入有效的远程名称".into());
    }
    let root = repository(root)?;
    if !run(&root, &["remote"])?.lines().any(|item| item == name) {
        return Err("远程仓库不存在".into());
    }
    run(&root, &["remote", "remove", "--", name])?;
    Ok(())
}
pub fn sync(root: &str, name: &str, push: bool, branch: Option<&str>) -> Result<String, String> {
    let root = repository(root)?;
    let name = remote(&root, name)?;
    let branch = match branch.map(str::trim).filter(|value| !value.is_empty()) {
        Some(chosen) => chosen.to_string(),
        None => run(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .map_err(|_| "请先返回本地分支".to_string())?,
    };
    if push {
        run(&root, &["push", "--set-upstream", "--", name, &branch])
    } else {
        // Never silently merge, rewrite history or discard uncommitted changes.
        run(&root, &["pull", "--ff-only", "--", name, &branch])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git;
    use std::{fs, process::Command};
    #[test]
    fn push_pull_and_divergence_preserve_local_history() {
        let t = tempfile::tempdir().unwrap();
        let bare = t.path().join("remote.git");
        assert!(Command::new("git")
            .args(["init", "--bare", "--initial-branch=main"])
            .arg(&bare)
            .status()
            .unwrap()
            .success());
        let a = t.path().join("a");
        fs::create_dir(&a).unwrap();
        let root = a.to_str().unwrap();
        git::init(root).unwrap();
        fs::write(a.join("a.txt"), "a").unwrap();
        git::stage(root, &[], true).unwrap();
        git::commit(root, "first", "Test", "test@example.com").unwrap();
        save(root, "origin", bare.to_str().unwrap()).unwrap();
        sync(root, "origin", true, None).unwrap();
        assert_eq!(status(root).unwrap().upstream, "origin/main");
        let b = t.path().join("b");
        assert!(Command::new("git")
            .arg("clone")
            .arg(&bare)
            .arg(&b)
            .status()
            .unwrap()
            .success());
        fs::write(b.join("b.txt"), "remote").unwrap();
        git::stage(b.to_str().unwrap(), &[], true).unwrap();
        git::commit(b.to_str().unwrap(), "remote", "Test", "test@example.com").unwrap();
        sync(b.to_str().unwrap(), "origin", true, None).unwrap();
        sync(root, "origin", false, None).unwrap();
        assert_eq!(fs::read_to_string(a.join("b.txt")).unwrap(), "remote");
        fs::write(a.join("a.txt"), "local").unwrap();
        git::stage(root, &[], true).unwrap();
        let before = git::commit(root, "local", "Test", "test@example.com").unwrap();
        fs::write(b.join("c.txt"), "remote2").unwrap();
        git::stage(b.to_str().unwrap(), &[], true).unwrap();
        git::commit(b.to_str().unwrap(), "remote2", "Test", "test@example.com").unwrap();
        sync(b.to_str().unwrap(), "origin", true, None).unwrap();
        assert!(sync(root, "origin", false, None).is_err());
        assert!(sync(root, "origin", true, None).is_err());
        assert_eq!(git::refs(root).unwrap().head, before);
        assert!(save(root, "--bad", "x").is_err());
        assert!(sync(root, "missing", true, None).is_err());
    }
}
