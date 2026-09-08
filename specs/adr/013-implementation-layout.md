# ADR-013: границы исходного кода

Дата: 2026-09-08. Решение реализации в рамках принятой спецификации.

Rust domain, storage, scheduler, Codex adapter и gateway начинаются отдельными модулями одной библиотеки `polymas-core`. Исполняемая служба — отдельный crate; Tauri — отдельный клиент. Это сохраняет логические границы из раздела 03, общие транзакции и позволяет выделять crates без изменения wire contract.

Первый UI использует React/TypeScript/Vite и Sigma/Graphology. Desktop IPC остаётся отдельным от dev browser transport. Данные workspace, credentials и результаты реального Codex не включаются в Git.
