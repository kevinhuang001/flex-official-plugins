//! Fiex Git plugin runs as an independent process; never links to Fiex Core.
mod files;
mod git;
mod git_history;
mod git_remote;
mod ignores;

use fiex_plugin_sdk::{self as sdk, RpcError};
use serde_json::{json, Value};

fn param<'a>(p: &'a Value, name: &str) -> Result<&'a str, RpcError> {
    p.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params(format!("missing {name}")))
}
fn string_list(p: &Value, name: &str) -> Result<Vec<String>, RpcError> {
    let values = p
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| RpcError::invalid_params(format!("missing {name}")))?;
    values
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| RpcError::invalid_params("invalid string list"))
        })
        .collect()
}
fn response<T: serde::Serialize>(result: Result<T, String>) -> Result<Value, RpcError> {
    serde_json::to_value(result.map_err(RpcError::internal)?)
        .map_err(|e| RpcError::internal(e.to_string()))
}
fn dispatch(method: &str, p: Value) -> Result<Value, RpcError> {
    let root = || param(&p, "root");
    match method {
        "git.status" => Ok(json!(git::status(root()?))),
        "git.init" => response(git::init(root()?)),
        "git.refs" => response(git::refs(root()?)),
        "git.identity" => response(Ok(git::identity(root()?))),
        "git.stage" => response(git::stage(
            root()?,
            &string_list(&p, "paths")?,
            p.get("all").and_then(Value::as_bool).unwrap_or(false),
        )),
        "git.unstage" => response(git::unstage(
            root()?,
            &string_list(&p, "paths")?,
            p.get("all").and_then(Value::as_bool).unwrap_or(false),
        )),
        "git.commit" => response(git::commit(
            root()?,
            param(&p, "message")?,
            param(&p, "name")?,
            param(&p, "email")?,
        )),
        "git.history" => response(git_history::history(
            root()?,
            p.get("skip").and_then(Value::as_u64).unwrap_or(0) as usize,
        )),
        "git.browse" => response(git_history::browse(root()?, param(&p, "reference")?)),
        "git.historyAsset" => response(git_history::asset(param(&p, "path")?)),
        "git.historyList" => response(git_history::list(
            param(&p, "path")?,
            p.get("hidden").and_then(Value::as_bool).unwrap_or(false),
            &string_list(&p, "ignores")?,
        )),
        "git.historyInfo" => response(git_history::info(param(&p, "path")?)),
        "git.historyPreview" => response(git_history::preview(param(&p, "path")?)),
        "git.historySearch" => response(git_history::search(
            param(&p, "path")?,
            param(&p, "query")?,
            p.get("hidden").and_then(Value::as_bool).unwrap_or(false),
            &string_list(&p, "ignores")?,
        )),
        "git.remoteStatus" => response(git_remote::status(root()?)),
        "git.remoteSave" => response(git_remote::save(
            root()?,
            param(&p, "name")?,
            param(&p, "url")?,
        )),
        "git.remoteRemove" => response(git_remote::remove(root()?, param(&p, "name")?)),
        "git.remoteSync" => response(git_remote::sync(
            root()?,
            param(&p, "remote")?,
            p.get("push").and_then(Value::as_bool).unwrap_or(false),
            p.get("branch").and_then(Value::as_str),
        )),
        // Core VFS uses standard methods; legacy history:// is contained in plugin.
        "fs.list" => {
            let path = param(&p, "directoryId")?;
            let result = git_history::list(path, true, &[]).map_err(RpcError::internal)?;
            let start = p
                .get("cursor")
                .and_then(Value::as_str)
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| RpcError::invalid_params("invalid cursor"))?;
            let limit = p
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(200)
                .clamp(1, 500) as usize;
            if start > result.entries.len() {
                return Err(RpcError::invalid_params("cursor out of range"));
            }
            let end = (start + limit).min(result.entries.len());
            let entries = result.entries[start..end]
                .iter()
                .map(|entry| {
                    json!({
                        "nodeId":entry.path,"name":entry.name,
                        "kind":if entry.is_dir {"directory"} else {"file"},
                        "size":entry.size,"modified":entry.modified,"capabilities":["read"]
                    })
                })
                .collect::<Vec<_>>();
            Ok(
                json!({"entries":entries,"nextCursor":if end<result.entries.len(){Some(end.to_string())}else{None},"snapshotToken":null}),
            )
        }
        "fs.stat" => response(git_history::info(param(&p, "nodeId")?)),
        _ => Err(RpcError::method_not_found(method)),
    }
}
fn main() {
    if let Err(e) = sdk::serve("org.fiex.git", dispatch) {
        eprintln!("Git plugin IPC error: {e}");
        std::process::exit(1);
    }
}
