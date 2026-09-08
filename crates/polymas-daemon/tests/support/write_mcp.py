"""Real STDIO MCP fixture. Writes only the test-owned path passed by the harness."""
import json
from pathlib import Path
import sys
import time

target = Path(sys.argv[1])
mode = sys.argv[2]
for line in sys.stdin:
    request = json.loads(line)
    if "id" not in request:
        continue
    method = request["method"]
    if method == "initialize":
        result = {"protocolVersion": "2025-03-26", "capabilities": {"tools": {}}, "serverInfo": {"name": "polymas-write-fixture", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": [{"name": "write", "description": "Append a fixture record", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}}]}
    elif method == "tools/call":
        arguments = request["params"]["arguments"]
        with target.open("a", encoding="utf-8") as output:
            output.write(json.dumps(arguments, ensure_ascii=False) + "\n")
        if mode == "disconnect":
            sys.exit(0)
        if mode == "slow":
            time.sleep(60)
        result = {"content": [{"type": "text", "text": "WROTE:" + arguments["text"]}], "isError": mode == "error"}
    elif method == "ping":
        result = {}
    else:
        print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "error": {"code": -32601, "message": "unsupported"}}), flush=True)
        continue
    print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)
