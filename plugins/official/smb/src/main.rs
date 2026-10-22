//! SMB network protocol adapter. Mount registry, cache and sync owned by host.
use fiex_plugin_sdk::{self as sdk, RpcError};
use futures_util::{pin_mut, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use smb::ReadAt;
use std::{fs, io::Write, path::Path, str::FromStr};
use tokio::runtime::Builder;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredConnection {
    host: String,
    share: String,
    #[serde(default)]
    base: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    port: Option<u16>,
}
struct RemoteFile {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: u64,
}
fn remote_join(base: &str, rel: &str) -> String {
    [base.trim_matches('/'), rel.trim_matches('/')]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}
fn safe_relative(value: &str) -> Result<(), RpcError> {
    if value.starts_with('/')
        || value
            .split('/')
            .any(|p| p == "." || p == ".." || p.contains(['\\', '\0']))
    {
        return Err(RpcError::invalid_params("invalid node ID"));
    }
    Ok(())
}
fn dispatch(method: &str, p: Value) -> Result<Value, RpcError> {
    let config: StoredConnection = serde_json::from_value(
        p.get("connection")
            .cloned()
            .ok_or_else(|| RpcError::invalid_params("missing connection"))?,
    )
    .map_err(|e| RpcError::invalid_params(e.to_string()))?;
    let path = p
        .get("directoryId")
        .or_else(|| p.get("nodeId"))
        .and_then(Value::as_str)
        .unwrap_or("");
    safe_relative(path)?;
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| RpcError::internal(e.to_string()))?;
    match method {
        "connection.validate" => {
            rt.block_on(smb_list(&config, ""))
                .map_err(RpcError::internal)?;
            Ok(json!({"valid":true}))
        }
        "fs.list" => {
            let mut nodes = rt
                .block_on(smb_list(&config, path))
                .map_err(RpcError::internal)?;
            nodes.sort_by(|a, b| a.name.cmp(&b.name));
            let start = p
                .get("cursor")
                .and_then(Value::as_str)
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| RpcError::invalid_params("invalid cursor"))?;
            if start > nodes.len() {
                return Err(RpcError::invalid_params("cursor out of range"));
            }
            let limit = p
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(200)
                .clamp(1, 500) as usize;
            let end = (start + limit).min(nodes.len());
            let entries=nodes[start..end].iter().map(|f|json!({
                "nodeId":if path.is_empty(){f.name.clone()}else{format!("{path}/{}",f.name)},
                "name":f.name,"kind":if f.is_dir{"directory"}else{"file"},
                "size":f.size,"modified":f.modified
            })).collect::<Vec<_>>();
            Ok(
                json!({"entries":entries,"nextCursor":if end<nodes.len(){Some(end.to_string())}else{None},
                "snapshotToken":null,"identityStability":"path"}),
            )
        }
        _ => Err(RpcError::method_not_found(method)),
    }
}
fn main() {
    if let Err(e) = sdk::serve("org.fiex.smb", dispatch) {
        eprintln!("SMB plugin IPC error: {e}");
        std::process::exit(1);
    }
}

async fn smb_client(conn: &StoredConnection) -> Result<(smb::Client, smb::UncPath), String> {
    let mut config = smb::ClientConfig::default();
    if let Some(port) = conn.port {
        config.connection.port = Some(port);
    }
    let client = smb::Client::new(config);
    let unc = smb::UncPath::from_str(&format!("\\\\{}\\{}", conn.host, conn.share))
        .map_err(|e| e.to_string())?;
    client
        .share_connect(&unc, &conn.username, conn.password.clone())
        .await
        .map_err(|e| format!("SMB 连接失败：{e}"))?;
    Ok((client, unc))
}

fn smb_path(conn: &StoredConnection, rel: &str) -> String {
    let joined = remote_join(&conn.base, rel).replace('/', "\\");
    joined.trim_matches('\\').to_string()
}

async fn smb_list(conn: &StoredConnection, rel: &str) -> Result<Vec<RemoteFile>, String> {
    let (client, unc) = smb_client(conn).await?;
    let path = smb_path(conn, rel);
    let target = if path.is_empty() {
        unc.clone()
    } else {
        unc.with_path(&path)
    };
    let dir = client
        .create_file(
            &target,
            &smb::FileCreateArgs::make_open_existing(
                smb::DirAccessMask::new()
                    .with_list_directory(true)
                    .with_synchronize(true)
                    .into(),
            ),
        )
        .await
        .map_err(|e| e.to_string())?
        .unwrap_dir();
    let dir = std::sync::Arc::new(dir);
    let stream =
        smb::Directory::query_with_options::<smb::FileFullDirectoryInformation>(&dir, "*", 65536)
            .await
            .map_err(|e| e.to_string())?;
    pin_mut!(stream);
    let mut out = Vec::new();
    while let Some(item) = stream.next().await {
        let item = item.map_err(|e| e.to_string())?;
        let name = item.file_name.to_string();
        if name == "." || name == ".." {
            continue;
        }
        out.push(RemoteFile {
            name,
            path: String::new(),
            is_dir: item.file_attributes.directory(),
            size: item.end_of_file,
            modified: 0,
        });
    }
    let _ = dir.close().await;
    Ok(out)
}

async fn smb_get(conn: &StoredConnection, rel: &str, dest: &Path) -> Result<u64, String> {
    let (client, unc) = smb_client(conn).await?;
    let path = smb_path(conn, rel);
    let file = client
        .create_file(
            &unc.with_path(&path),
            &smb::FileCreateArgs::make_open_existing(
                smb::FileAccessMask::new().with_generic_read(true),
            ),
        )
        .await
        .map_err(|e| e.to_string())?
        .unwrap_file();
    let mut output = fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut offset = 0u64;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut total = 0u64;
    loop {
        let read = file
            .read_at(&mut buffer, offset)
            .await
            .map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|e| e.to_string())?;
        offset += read as u64;
        total += read as u64;
    }
    let _ = file.close().await;
    Ok(total)
}
