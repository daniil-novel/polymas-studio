# Development contract

Read `TASKS.md`, the relevant specification section, and current Git status before editing.
Preserve agent identity: one active Codex process per AgentInstance; never share a session between agents.
Write a meaningful failing regression test before each behavior change, then implement and run it.
One commit per small feature or fix. Review staged diff; run the relevant suite before AND after committing.
Keep CHANGELOG.md, TASKS.md and acceptance evidence truthful. Do not call synthetic runs real provider tests.
Never commit secrets, live workspaces, private task output, credentials or logs. Use fixtures without personal data.
Never disable sandbox/approvals to get a test passing. Missing isolation/capacity is an admission error.
Do not rewrite accepted requirements to hide unfinished work. Record deviations in an ADR and expose limitations.
Use the supplied design tokens; validate behavior and visual layout in the actual UI.
