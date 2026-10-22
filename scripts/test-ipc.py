#!/usr/bin/env python3
"""End-to-end check of the exact framed protocol used by Fiex PluginHost.

Launches the actual native plugin binary, rather than importing its Rust code.
"""
import argparse
import json
import subprocess

def request(process, number, method, params=None):
    value = json.dumps({"jsonrpc":"2.0","id":number,"method":method,
                        "params":params or {}},separators=(",",":")).encode()
    process.stdin.write(b"Content-Length: "+str(len(value)).encode()+b"\r\n\r\n"+value)
    process.stdin.flush()
    headers = {}
    while True:
        line = process.stdout.readline()
        if not line:
            raise AssertionError("Plugin exited without replying")
        if line == b"\r\n":
            break
        name, sep, data = line.partition(b":")
        assert sep, "Invalid RPC frame header"
        headers[name.lower()] = data.strip()
    length = int(headers[b"content-length"])
    assert 0 <= length <= 4 * 1024 * 1024
    payload = process.stdout.read(length)
    assert len(payload) == length
    reply = json.loads(payload)
    assert reply.get("id") == number, reply
    assert reply.get("jsonrpc") == "2.0", reply
    return reply

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--binary",required=True)
    parser.add_argument("--plugin-id",required=True)
    args=parser.parse_args()
    process = subprocess.Popen([args.binary],stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        result=request(process,1,"plugin.initialize",{"protocolVersion":"1.0"})
        assert result["result"]["pluginId"] == args.plugin_id, result
        assert result["result"]["ready"] is True, result
        assert request(process,2,"plugin.ping")["result"]["alive"] is True
        response=request(process,3,"example.unknown")
        assert isinstance(response.get("error"),dict),response
        assert request(process,4,"plugin.shutdown")["result"] is None
        assert process.wait(timeout=10) == 0
        print("Native host IPC verified:",args.plugin_id)
    finally:
        if process.poll() is None: process.kill()
        process.communicate(timeout=10)
if __name__ == "__main__":
    main()
