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
| D09b | `mcp_client` отсутствует, E0432 | Реальный STDIO MCP fixture: initialize → tools/list → tools/call; disabled/unlisted rejected | Neo4j endpoint/credentials пока не предоставлены; HTTP отдельный integration сценарий |
| D09c | `secrets` отсутствует, E0432 | Secret Debug redaction; opt-in Windows Credential Manager save/load/delete собственной fixture записи | Нативный test исключён из обычного CI, выполнен отдельно на хосте |
| D10a | `policy` отсутствует, E0432 | Single-use после reopen; изменённые аргументы/epoch/expiry отклоняются; отказ не исполняется | Gateway связывает ledger с фактическим вызовом отдельным срезом |
| D10b | Binding/authorize_tool отсутствуют, E0432 | Revocation, новые grants, чужой агент, disabled MCP, unlisted tool отклоняются | Namespace проверяет gateway; raw DB principal требует отдельной изоляции |
| D11 | `knowledge` отсутствует, E0432 | Provenance после reopen; неизменяемая версия источника; namespace isolation; retention; явная проверка и contradicts | Утверждения не становятся истинными автоматически; UI/gateway далее |
| D12a | `artifacts` отсутствует, E0432 | Неизменяемые версии, обнаружение corrupt blob, запрет импорта за пределами workspace | Максимум 64 МиБ на артефакт; preview реализуется в UI |
| D12b | `backup` отсутствует, E0432 | Snapshot не включает последующие события; blobs восстанавливаются с hash; corrupt database отклоняется до публикации; существующая папка не перезаписывается | Двухфайловая fixture, не NFR-10 benchmark; OS secrets и provider sessions в backup не входят |
| D13a | `workspace` отсутствует, E0432 | Роли/10 экземпляров сохраняются после reopen; stale revision и invalid draft не меняют данные; MCP draft не может подделать Ready | Transport и runtime loop следуют отдельными срезами |
| D13a fix | Реальный Windows Junction в `agents` обходил проверку containment; regression FAILED | Exact directory identity до создания workspace, parent/agent aliases отклоняются, второй service не получает lock | Это проверка путей перед launch, не полная сертификация sandbox |
| D13b | `engine` отсутствует, E0432 | Claim → restart → unknown; нет автоповтора; pinned role; acceptance после reopen; active archive/stale fence отклонены | Реальный исполнительный loop подключается следующим срезом |
| D13c | AppState/router отсутствуют, E0432 | Missing token/foreign Origin → 401; авторизованное редактирование → 200; stale revision → 409; данные читаются через HTTP router | Локальный owner transport; remote RBAC отдельный этап |
| D07d | Regression FAILED: отсутствовало подтверждение stdin | Одно событие после write/shutdown; broken input pipe отклоняется при success output | Подтверждает передачу байтов процессу, не обработку сообщения моделью; ack остаётся отдельным |

Реальные provider тесты выполняются только отдельно, не в обычном CI. Отчёты не содержат credentials и приватных задач. Production matrix будет записана в `acceptance.md`.
