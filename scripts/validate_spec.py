"""Validate the specification package and its synthetic contract examples."""

from __future__ import annotations

import json
import re
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker

ROOT = Path(__file__).resolve().parents[1]


def validate_architecture(document: dict) -> None:
    indexed = {}
    for collection in ("roles", "populations", "holons", "connections", "bindings"):
        values = document[collection]
        ids = [value["id"] for value in values]
        assert len(ids) == len(set(ids)), f"Duplicate IDs: {collection}"
        indexed[collection] = {value["id"]: value for value in values}
    total = sum(value["count"] for value in document["populations"])
    assert 1 <= total <= 5000, "Invalid total instances"
    assert document["execution"]["maxConcurrency"] <= total, "Concurrency exceeds N"
    for population in document["populations"]:
        role = indexed["roles"][population["roleId"]]
        assert role["version"] == population["roleVersion"], "Wrong role version"
    coordinator = document["topology"]["coordinatorPopulationId"]
    if coordinator is not None:
        assert indexed["populations"][coordinator]["count"] == 1, "Coordinator must be one instance"
    if document["topology"]["preset"] == "single":
        assert total == 1 and coordinator is None, "Single preset invariant"
    if document["topology"]["preset"] == "star":
        assert coordinator is not None, "Star needs a coordinator"
    owners = set()
    for holon in document["holons"]:
        for population_id in holon["populationIds"]:
            assert population_id in indexed["populations"]
            assert population_id not in owners, "Duplicate holon owner"
            owners.add(population_id)
        seen = {holon["id"]}
        parent = holon["parentId"]
        while parent is not None:
            assert parent not in seen, "Holon ownership cycle"
            seen.add(parent)
            parent = indexed["holons"][parent]["parentId"]
    for edge in document["topology"]["edges"]:
        for endpoint in ("from", "to"):
            kind, identifier = edge[endpoint].split(":", 1)
            collection = {"population": "populations", "holon": "holons"}[kind]
            assert identifier in indexed[collection], "Unknown edge endpoint"
    for binding in document["bindings"]:
        assert binding["connectionId"] in indexed["connections"]
        kind, identifier = binding["subject"].split(":", 1)
        assert identifier in indexed[{"role": "roles", "population": "populations", "holon": "holons"}[kind]]
    memory = document["topology"]["sharedMemoryConnectionId"]
    if memory is not None:
        assert indexed["connections"][memory]["enabledDesired"]
        assert any(binding["connectionId"] == memory for binding in document["bindings"])


def main() -> None:
    validators = {}
    for name in ("architecture", "event"):
        schema = json.loads((ROOT / "contracts" / f"{name}.schema.json").read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(schema)
        validators[name] = Draft202012Validator(schema, format_checker=FormatChecker())
    examples = sorted((ROOT / "contracts" / "examples").glob("*.json"))
    for example in examples:
        document = json.loads(example.read_text(encoding="utf-8"))
        validators["architecture"].validate(document)
        validate_architecture(document)
    events = (ROOT / "contracts/examples/events.jsonl").read_text(encoding="utf-8").splitlines()
    seen_ids = set()
    streams = {}
    for line in filter(None, events):
        event = json.loads(line)
        validators["event"].validate(event)
        assert event["eventId"] not in seen_ids
        seen_ids.add(event["eventId"])
        assert event["sequence"] > streams.get(event["streamId"], 0)
        streams[event["streamId"]] = event["sequence"]
    checked_links = 0
    for path in ROOT.rglob("*.md"):
        for target in re.findall(r"!?\[[^\]]*\]\(([^)]+)\)", path.read_text(encoding="utf-8")):
            if "://" in target or target.startswith("#"):
                continue
            target = target.split("#")[0].strip("<>")
            assert (path.parent / target).is_file(), f"Broken link: {path.relative_to(ROOT)} -> {target}"
            checked_links += 1
    print(f"PASS: 2 schemas, {len(examples)} architectures, {len(seen_ids)} events, {checked_links} local links")


if __name__ == "__main__":
    main()
