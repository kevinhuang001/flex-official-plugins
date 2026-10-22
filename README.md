# Flex Official Plugins

Independently distributed Fiex plugins. **Nothing is bundled with the Fiex desktop installer.**
Each plugin is installed from a GitHub Release after explicit user permission approval; official plugins receive no special privileges.

| Plugin | Package | Purpose |
| --- | --- | --- |
| Git | `plugins/official/git` | Working tree, history and remotes |
| WebDAV | `plugins/official/webdav` | Network filesystem |
| SMB | `plugins/official/smb` | SMB network shares |
| Archive | `plugins/official/archive` | Compression/decompression |
| Checksum | `plugins/official/checksum` | File hashes |

## Independent tests
`cargo test --manifest-path plugins/official/<name>/Cargo.toml` checks plugin logic.
`scripts/test-ipc.py` spawns each native executable and checks the complete Fiex JSON-RPC lifecycle.
The CI matrix runs all five binaries on Linux, macOS and Windows and packages them as individual `.fiexplugin` files. The additional Core-host integration job checks actual PluginHost spawning/approval against a separately checked-out Core build.

The pinned Rust IPC SDK is maintained in [flex-sdk](https://github.com/kevinhuang001/flex-sdk).
The Fiex Core/host UI is maintained in [fiex](https://github.com/kevinhuang001/fiex).
Builds pin an exact SDK Git commit and never use relative SDK paths.
