"""Opt-in real provider check through the authenticated daemon HTTP API."""
import argparse
import json
import os
from pathlib import Path
import queue
import secrets
import subprocess
import threading
import time
import urllib.request
import uuid


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--allow-provider", action="store_true")
    args = parser.parse_args()
    if not args.allow_provider:
        parser.error("This test uses the signed-in Codex account; pass --allow-provider")
    root = Path(__file__).resolve().parent.parent
    workspace = root / "tmp" / "service-smoke" / str(uuid.uuid4())
    workspace.mkdir(parents=True)
    token = secrets.token_hex(32)
    env = dict(os.environ, POLYMAS_CONTROL_TOKEN=token, NO_PROXY="127.0.0.1,localhost")
    binary = root / "target" / "debug" / ("polymas-daemon.exe" if os.name == "nt" else "polymas-daemon")
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    with (workspace / "service.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen([str(binary), "--workspace", str(workspace), "--port", "0"],
                                   env=env, stdout=subprocess.PIPE, stderr=log, text=True,
                                   encoding="utf-8", creationflags=flags)
        endpoint = None
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

        def request(path, body=None):
            req = urllib.request.Request(endpoint + path, data=None if body is None else json.dumps(body).encode(),
                                         headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
            with opener.open(req, timeout=15) as response:
                return json.load(response)

        try:
            ready = queue.Queue()
            threading.Thread(target=lambda: ready.put(process.stdout.readline()), daemon=True).start()
            startup = json.loads(ready.get(timeout=20))
            endpoint = f"http://127.0.0.1:{startup['port']}"
            for _ in range(60):
                runtime = request("/api/runtime")
                if runtime["ready"]:
                    break
                if runtime["error"]:
                    raise RuntimeError(runtime["error"])
                time.sleep(0.5)
            else:
                raise TimeoutError("Runtime probe did not finish")
            role = request("/api/configure", {"revision": 1, "command": {"type": "create_role", "draft": {
                "name": "Service check", "instruction": "Выполни короткую текстовую задачу. Не используй инструменты.",
                "timeoutSeconds": 120}}})["result"]
            request("/api/configure", {"revision": 2, "command": {"type": "create_agents", "key": "one", "request": {
                "roleId": role["id"], "roleVersion": 1, "count": 1, "namePattern": "Проверка"}}})
            pids, sessions = [], []
            for i, prompt in enumerate(["Запомни маркер POLYMAS-SERVICE-71. Ответь только этим маркером.",
                                        "Повтори маркер из предыдущей задачи. Ответь только маркером."]):
                run = request("/api/runs", {"key": f"run-{i}", "input": {"title": f"Check {i + 1}", "prompt": prompt}})
                task_id = run["taskIds"][0]
                for _ in range(240):
                    task = request("/api/tasks/" + task_id)
                    if task["state"] == "awaiting_acceptance":
                        break
                    if task["state"] in {"failed", "cancelled", "unknown"}:
                        raise RuntimeError(task["error"])
                    time.sleep(0.5)
                else:
                    raise TimeoutError("Codex result timeout")
                assert task["result"].strip() == "POLYMAS-SERVICE-71", "Unexpected provider result"
                assert len(task["artifactIds"]) == 1
                pids.append(task["lease"]["pid"])
                snapshot = request("/api/state")
                sessions.append(snapshot["agentRuntime"][0]["value"]["sessionId"])
                request("/api/tasks/" + task_id + "/action", {"action": "accept"})
            assert pids[0] != pids[1], "Each attempt must use its own process"
            assert sessions[0] == sessions[1], "Explicit resume must preserve this agent session"
            print(json.dumps({"passed": True, "codexVersion": runtime["probe"]["version"],
                              "authMethod": runtime["probe"]["authMethod"], "pids": pids,
                              "sameAgentSession": True, "artifacts": 2, "acceptedTasks": 2}))
        finally:
            if endpoint:
                try:
                    request("/api/control", {"action": "shutdown"})
                except (OSError, urllib.error.URLError):
                    pass
            try:
                process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=5)


if __name__ == "__main__":
    main()
