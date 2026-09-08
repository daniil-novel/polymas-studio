# Проверки реализации

Тесты выполняются до и после каждого функционального коммита. RED означает наблюдавшееся падение нужного сценария до реализации; GREEN — его успешный прогон после изменения. История коммитов сохраняет законченные срезы, а не намеренно сломанные сборки.

| Срез | RED / основание | GREEN | Ограничения |
|---|---|---|---|
| D01 | Пакет спецификации уже существует, поведение не менялось | `python scripts/validate_spec.py`, 2 schemas / 3 architectures / 3 events / 43 links, до и после 57db2c9 | Не проверка runtime |
| D02 | Компилятор не находил Windows SDK; validator обходил node_modules | Rust compile/test, 2 frontend contract tests, TypeScript, ESLint, Vite build, schema validator | Windows 10 19045 фактического хоста; целевой Windows 11 проверяется отдельно |
| D03 | `cargo test --test registry`: отсутствует registry, E0432 | 4 regression scenarios: 1000 distinct IDs/workspaces, idempotency conflict, immutable versions, atomic invalid batch | Процессы добавляются в D07 |
| D04 | `cargo test --test storage`: storage отсутствует, E0432 | Reopen persistence, rollback без частичной записи, stale revision, monotonic replay cursor | Локальная SQLite, не сетевая файловая система |
| D05 | `cargo test --test topology`: topology отсутствует, E0432 | 4 сочетания ядра/памяти; один агент; cycles/ownership; explicit recursive dispatch | Проверка domain, UI следует отдельно |

Реальные provider тесты выполняются только отдельно, не в обычном CI. Отчёты не содержат credentials и приватных задач. Production matrix будет записана в `acceptance.md`.
