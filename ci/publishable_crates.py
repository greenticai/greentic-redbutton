#!/usr/bin/env python3
import json
import subprocess
import sys
from collections import defaultdict, deque

metadata = json.loads(
    subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"], text=True)
)
workspace_members = set(metadata["workspace_members"])
packages = {pkg["id"]: pkg for pkg in metadata["packages"] if pkg["id"] in workspace_members}
publishable = {
    pkg_id: pkg
    for pkg_id, pkg in packages.items()
    if pkg.get("publish", None) is not False
}

graph = defaultdict(set)
in_degree = {pkg_id: 0 for pkg_id in publishable}

for pkg_id, pkg in publishable.items():
    for dep in pkg.get("dependencies", []):
        dep_id = None
        for candidate_id, candidate in publishable.items():
            if candidate["name"] == dep["name"]:
                dep_id = candidate_id
                break
        if dep_id and dep_id != pkg_id and pkg_id not in graph[dep_id]:
            graph[dep_id].add(pkg_id)
            in_degree[pkg_id] += 1

queue = deque(sorted([pkg_id for pkg_id, degree in in_degree.items() if degree == 0], key=lambda item: publishable[item]["name"]))
ordered = []
while queue:
    pkg_id = queue.popleft()
    ordered.append(publishable[pkg_id]["name"])
    for neighbor in sorted(graph[pkg_id], key=lambda item: publishable[item]["name"]):
        in_degree[neighbor] -= 1
        if in_degree[neighbor] == 0:
            queue.append(neighbor)

if len(ordered) != len(publishable):
    print("cycle detected in publishable crates", file=sys.stderr)
    sys.exit(1)

for name in ordered:
    print(name)
