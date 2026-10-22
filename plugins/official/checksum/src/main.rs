mod checksums;
use fiex_plugin_sdk::{self as sdk, RpcError};
use serde_json::{json, Value};
fn run(method: &str, p: Value) -> Result<Value, RpcError> {
    match method {
        "checksum.calculate" => {
            let path = p
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_params("missing path"))?;
            Ok(json!(
                checksums::calculate(path).map_err(RpcError::internal)?
            ))
        }
        _ => Err(RpcError::method_not_found(method)),
    }
}
fn main() {
    if let Err(e) = sdk::serve("org.fiex.checksum", run) {
        eprintln!("Checksum plugin failed: {e}");
        std::process::exit(1);
    }
}
