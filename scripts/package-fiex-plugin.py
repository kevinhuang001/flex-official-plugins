#!/usr/bin/env python3
"""Create installable per-platform .fiexplugin packages."""
import argparse
import json
from pathlib import Path
import zipfile

TARGETS={"x86_64-pc-windows-msvc":"windows-x86_64",
         "aarch64-apple-darwin":"macos-aarch64",
         "x86_64-apple-darwin":"macos-x86_64",
         "x86_64-unknown-linux-gnu":"linux-x86_64"}
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--slug",required=True)
    parser.add_argument("--target",required=True)
    parser.add_argument("--target-dir",default="target")
    parser.add_argument("--output",default="dist/plugins")
    args=parser.parse_args()
    platform=TARGETS.get(args.target)
    if not platform:parser.error("unsupported Rust target")
    root=Path("plugins/official")/args.slug
    manifest_path=root/"manifest.json"
    manifest=json.loads(manifest_path.read_text(encoding="utf-8"))
    entry=manifest.get("entrypoints",{}).get(platform)
    if not entry or Path(entry).name!=entry:parser.error("invalid plugin entrypoint")
    binary=Path(args.target_dir)/args.target/"release"/entry
    if not binary.is_file():parser.error("missing compiled binary: "+str(binary))
    output=Path(args.output)
    output.mkdir(parents=True,exist_ok=True)
    archive_path=output/(args.slug+"-"+platform+".fiexplugin")
    with zipfile.ZipFile(archive_path,"w",compression=zipfile.ZIP_DEFLATED,compresslevel=6) as archive:
        archive.write(manifest_path,"manifest.json")
        archive.write(binary,entry)
        ui=root/"ui"
        if ui.is_dir():
            for file in sorted(ui.rglob("*")):
                if file.is_file() and not file.is_symlink():
                    archive.write(file,file.relative_to(root).as_posix())
    print("Created",archive_path,archive_path.stat().st_size,"bytes")
if __name__=="__main__":main()
