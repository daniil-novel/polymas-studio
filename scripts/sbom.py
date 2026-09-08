"""Validate and make generated CycloneDX evidence portable; never infer licenses."""
from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
import re
from typing import Any
from urllib.parse import quote


def components(document: dict[str, Any]) -> list[dict[str, Any]]:
    result = [document["metadata"]["component"]]
    pending = list(document.get("components", []))
    while pending:
        component = pending.pop()
        result.append(component)
        pending.extend(component.get("components", []))
    return result


def portable(document: dict[str, Any], name: str, version: str) -> dict[str, Any]:
    """Rewrite local Cargo references together with every edge, failing on leaks."""
    data = copy.deepcopy(document)
    if "signature" in data:
        raise ValueError("Normalize unsigned evidence before signing it")
    if data.get("bomFormat") != "CycloneDX" or data.get("specVersion") != "1.5":
        raise ValueError("Expected CycloneDX 1.5 evidence")
    revision = document.get("version", 1)
    if not isinstance(revision, int) or isinstance(revision, bool) or revision < 1:
        raise ValueError("Invalid SBOM revision")
    serial = data.get("serialNumber")
    if serial is not None and (
        not isinstance(serial, str)
        or not re.fullmatch(r"urn:uuid:[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", serial)
    ):
        raise ValueError("Invalid SBOM serial number")
    root = data.get("metadata", {}).get("component", {})
    # npm 11 uses the directory basename as display name even with --package-lock-only.
    if root.get("purl") == f"pkg:npm/{quote(name, safe='')}@{quote(version, safe='')}":
        root["name"] = name
    if root.get("name") != name or root.get("version") != version:
        raise ValueError("SBOM identity does not match the package")
    if not data.get("components") or not data.get("dependencies"):
        raise ValueError("Empty component inventory or dependency graph")
    items = components(data)
    refs: set[str] = set()
    replacements: dict[str, str] = {}
    for item in items:
        ref = item.get("bom-ref")
        if not isinstance(ref, str) or not ref or ref in refs:
            raise ValueError("Missing or duplicate component reference")
        refs.add(ref)
        if ref.startswith("path+file://"):
            if not item.get("name") or not item.get("version"):
                raise ValueError("Local component identity is incomplete")
            identity = f"{quote(item['name'], safe='')}@{quote(item['version'], safe='')}"
            replacement = f"urn:polymas:source:{identity}"
            if replacement in replacements.values() or replacement in refs:
                raise ValueError("Ambiguous local component identity")
            replacements[ref] = replacement
            item["bom-ref"] = replacement
            item["purl"] = f"pkg:cargo/{identity}"
    edges = data["dependencies"]
    edge_refs: set[str] = set()
    for edge in edges:
        ref = edge.get("ref")
        if ref not in refs or ref in edge_refs:
            raise ValueError("Invalid or duplicate dependency node")
        edge_refs.add(ref)
        edge["ref"] = replacements.get(ref, ref)
        for relation in ("dependsOn", "provides"):
            values = edge.get(relation, [])
            if not isinstance(values, list) or any(value not in refs for value in values):
                raise ValueError("Dependency refers to an unknown component")
            if relation in edge:
                edge[relation] = [replacements.get(value, value) for value in values]
    if root["bom-ref"] not in {edge["ref"] for edge in edges}:
        raise ValueError("Root component has no dependency graph")
    new_refs = [item["bom-ref"] for item in items]
    if len(set(new_refs)) != len(new_refs):
        raise ValueError("Normalized references collide")
    # Unknown path-bearing fields must be reviewed, not silently stripped.
    pending: list[Any] = [data]
    while pending:
        value = pending.pop()
        if isinstance(value, dict):
            pending.extend(value.values())
        elif isinstance(value, list):
            pending.extend(value)
        elif isinstance(value, str) and (
            "file://" in value.lower()
            or re.search(r"(?i)(?:^|\s)[a-z]:[/\\]", value)
            or re.search(r"/(?:Users|home)/", value)
        ):
            raise ValueError("Unexpected local machine path remains in SBOM")
    if data != document:
        data["version"] = revision + 1
    return data


def license_inventory(documents: list[dict[str, Any]]) -> list[dict[str, Any]]:
    inventory: dict[tuple[str, str, str], set[str]] = {}
    for document in documents:
        for item in components(document):
            key = (item["name"], item.get("version", ""), item.get("purl", ""))
            licenses = inventory.setdefault(key, set())
            for entry in item.get("licenses", []):
                label = (
                    entry.get("expression")
                    or entry.get("license", {}).get("id")
                    or entry.get("license", {}).get("name")
                )
                if label:
                    licenses.add(label)
    return [
        {
            "name": name, "version": version, "purl": purl,
            "licenses": sorted(licenses), "reviewRequired": not bool(licenses),
        }
        for (name, version, purl), licenses in sorted(inventory.items())
    ]


def read(path: Path) -> dict[str, Any]:
    if path.stat().st_size > 16 * 1024 * 1024:
        raise ValueError("SBOM exceeds 16 MiB")
    return json.loads(path.read_text(encoding="utf-8-sig"))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    normalize = commands.add_parser("normalize")
    normalize.add_argument("input", type=Path)
    normalize.add_argument("output", type=Path)
    normalize.add_argument("--name", required=True)
    normalize.add_argument("--version", required=True)
    inventory = commands.add_parser("licenses")
    inventory.add_argument("output", type=Path)
    inventory.add_argument("inputs", type=Path, nargs="+")
    args = parser.parse_args()
    try:
        inputs = [args.input] if args.command == "normalize" else args.inputs
        for path in inputs:
            if args.output.resolve() == path.resolve() or (
                args.output.exists() and args.output.samefile(path)
            ):
                raise ValueError("Output must not overwrite input evidence")
        result = (
            portable(read(args.input), args.name, args.version)
            if args.command == "normalize"
            else license_inventory([read(path) for path in args.inputs])
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
    except (ValueError, KeyError, TypeError, OSError) as error:
        parser.exit(1, f"SBOM evidence failed: {error}\n")


if __name__ == "__main__":
    main()
