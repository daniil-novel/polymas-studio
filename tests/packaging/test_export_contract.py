"""The export contract rejects private runtime/transport fields at every boundary."""
import copy
import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[2]


class ExportContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        schema = json.loads((ROOT / 'contracts/configuration-export.schema.json').read_text(encoding='utf-8'))
        Draft202012Validator.check_schema(schema)
        cls.validator = Draft202012Validator(schema)
        cls.example = json.loads((ROOT / 'contracts/export-examples/materialized.json').read_text(encoding='utf-8'))

    def test_exact_runtime_projections_validate(self):
        # Rust tests compare these complete fixtures against the actual serializer.
        for path in (ROOT / 'contracts/export-examples').glob('*.json'):
            with self.subTest(example=path.name):
                self.validator.validate(json.loads(path.read_text(encoding='utf-8')))

    def test_runtime_identity_and_credentials_cannot_be_added(self):
        for path, field in [
            ((), 'history'), ((), 'workspacePath'), (('runtime',), 'authProfileRef'),
            (('instances', 0), 'sessionId'), (('instances', 0), 'workspaceKey'),
            (('instances', 0), 'status'), (('roleVersions', 0), 'createdAt'),
            (('connections', 0), 'authSecretRef'), (('connections', 0), 'environment'),
            (('connections', 0), 'roots'), (('connections', 1), 'url'),
            (('connections', 2), 'args'), (('connections', 2), 'command'),
        ]:
            with self.subTest(path=path, field=field):
                document = copy.deepcopy(self.example)
                target = document
                for key in path:
                    target = target[key]
                target[field] = 'PRIVATE-FIXTURE'
                self.assertFalse(self.validator.is_valid(document))

    def test_unknown_versions_and_auto_activation_are_rejected(self):
        for path, value in [
            (('schemaVersion',), '99.0.0'), (('migrationVersion',), 99),
            (('runtime', 'adapter'), 'codex-app-server-experimental'),
            (('runtime', 'setupRequired'), False), (('architecture', 'paused'), False),
            (('connections', 0, 'setupRequired'), False),
        ]:
            with self.subTest(path=path):
                document = copy.deepcopy(self.example)
                target = document
                for key in path[:-1]:
                    target = target[key]
                target[path[-1]] = value
                self.assertFalse(self.validator.is_valid(document))
