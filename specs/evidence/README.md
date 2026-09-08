# Проверки реализации

Тесты выполняются до и после каждого функционального коммита. RED означает наблюдавшееся падение нужного сценария до реализации; GREEN — его успешный прогон после изменения. История коммитов сохраняет законченные срезы, а не намеренно сломанные сборки.

| Срез | RED / основание | GREEN | Ограничения |
|---|---|---|---|
| D01 | Пакет спецификации уже существует, поведение не менялось | `python scripts/validate_spec.py`, 2 schemas / 3 architectures / 3 events / 43 links, до и после 57db2c9 | Не проверка runtime |
| D02 | Компилятор не находил Windows SDK; validator обходил node_modules | Rust compile/test, 2 frontend contract tests, TypeScript, ESLint, Vite build, schema validator | Windows 10 19045 фактического хоста; целевой Windows 11 проверяется отдельно |
| D03 | `cargo test --test registry`: отсутствует registry, E0432 | 4 regression scenarios: 1000 distinct IDs/workspaces, idempotency conflict, immutable versions, atomic invalid batch | Процессы добавляются в D07 |
| D04 | `cargo test --test storage`: storage отсутствует, E0432 | Reopen persistence, rollback без частичной записи, stale revision, monotonic replay cursor | Локальная SQLite, не сетевая файловая система |
| D05 | `cargo test --test topology`: topology отсутствует, E0432 | 4 сочетания ядра/памяти; один агент; cycles/ownership; explicit recursive dispatch | Проверка domain, UI следует отдельно |
| D06 | `cargo test --test scheduler`: scheduler отсутствует, E0432 | N=1000/C=10; reject start-all при 10 slots; stale fence; acceptance/quorum; crash=unknown; no concurrent turns | Lease tests не заменяют OS process test |
| D07a | `cargo test --test codex_contract`: codex отсутствует, E0432 | Аргументы без shell/prompt interpolation; exact resume; UTF-8 chunks; malformed/oversize rejection | Реальный process следующий срез |
| D07b | `process` отсутствует; дополнительный RED: закрытые stdout/stderr отключали cancel, timeout 2 s | 4 tests с реальными Python child processes: разные PID, cancel, malformed JSON, closed-pipe cancellation | Контроль жизненного цикла, не доказательство hostile sandbox |
| D07c | CodexStream отсутствует, E0432 | 6 contract tests; реальный Codex 0.144.1 / ChatGPT: PID 19872 → 24424, explicit session resume, контрольный маркер сохранён | Два коротких turn, без tool use; не 1000-concurrency и не sandbox certification |
| D08 | `messages` отсутствует, E0432 | Повтор и restart без дубля; TTL; missing edge; hops; защита reply identity | Delivery API вызывается только после подтверждённой передачи контекста |
| D09a | `connections` отсутствует, E0432 | Untested toggle, URL credentials/plain HTTP rejection, read-only file root and parent traversal | Файловый доступ через cap-std; внешние MCP проверяются следующим срезом |

Реальные provider тесты выполняются только отдельно, не в обычном CI. Отчёты не содержат credentials и приватных задач. Production matrix будет записана в `acceptance.md`.
