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
| D07d follow-up | Проверка после ed8cb82 выявила FAILED broken-pipe: Windows Tokio shutdown не flush-ит буфер | Явный flush ожидает OS write и возвращает broken-pipe; все 5 process scenarios | Исправление отдельным коммитом, не переписанной историей |
| D13d | `runtime` отсутствует, E0432 | Реальный Python process через execution pipeline: PID, JSONL, stdin receipt, UUID session, результат и hash artifact сохранены | Protocol fixture, не OpenAI inference; реальный Codex end-to-end следует отдельно |
| D13e | `/api/runs` возвращал 404; readiness/control regression FAILED | HTTP tests; `smoke_service.py --allow-provider`: реальный Codex 0.144.1 / ChatGPT, PID 28760 → 29520, точный resume, 2 artifacts, 2 принятые задачи | 2 коротких turn без инструментов; не concurrency/soak gate |
| D14a | App/API imports отсутствовали, Vitest FAILED | UI не выдумывает агентов/активность при empty registry; unauthorized state видим; TS/lint/4 tests/build | Визуальная сверка и полноценные формы следуют отдельными срезами |
| D03 follow-up | Имена оставляли literal `{n:02}`, registry regression FAILED | Zero padding 01/02 и atomic reject неподдерживаемого шаблона | Все 5 registry tests |
| D15a | RoleEditor import отсутствовал; отдельный RED stale-role form | UI сначала сохраняет роль, затем создаёт точное N по полученным ID/revision; внешняя новая версия блокирует старую форму | Browser/computer-use выполняется на интегрированном приложении отдельно |

Реальные provider тесты выполняются только отдельно, не в обычном CI. Отчёты не содержат credentials и приватных задач. Production matrix будет записана в `acceptance.md`.

### D15 visual follow-up

D10 runtime event review: RED отсутствовала отдельная observable projection. GREEN MCP arguments/result/error content не копируются в event, имя/status сохранены, reasoning исключён, обычный progress видим. Семь codex contract tests и clippy. Это защита новых событий; провайдер может хранить собственную сессию отдельно от PolyMAS.

D10 actual Codex finding: PID 24540 обнаружил файловый источник, но Codex отклонил polymas_call_source до gateway как потенциальную запись (`user cancelled MCP tool call`). Это не успешный read smoke. RED нового regression показал отсутствие отдельного read interface. GREEN polymas_read_source объявлен read-only и отклоняет разрешённый write tool до call_started; пять gateway tests и clippy. Политика approval/sandbox не ослаблялась. [Codex MCP approval modes](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) проверены 2026-09-08.

D10 source credential review: RED отсутствовал sanitizer успешного MCP результата. GREEN вложенные content/structuredContent и ключи очищены от точных значений bearer/env secrets, обычный текст сохранён; два connection tests и clippy. Это защита от прямого echo, не обещание обнаружения произвольного кодирования секрета внешним сервером.

D10f: RED отсутствующий configure_gateway. GREEN execution fixture проверяет gateway launch/env, scoped context во время настоящего дочернего процесса и 401 после сохранения результата. Два runtime tests и clippy. Реальный Codex MCP smoke выполняется отдельно после коммита.

D10e: RED отдельный daemon --gateway-port закрывал initialize. GREEN: настоящий дочерний процесс bridge → MCP initialize/list/call → scoped HTTP → файловый marker; отзыв capability превращает следующий call в isError. Четыре gateway tests и clippy. Клиент теста — rmcp, реальный Codex проверяется после runtime wiring.

D10d: RED /gateway/call возвращал 404. GREEN: catalog с фильтром и лимитом → чтение fixture marker из реального файла; чужой namespace, traversal, impersonation и отозванный binding отклонены; call_completed есть, содержимого файла в events нет. Три gateway tests и clippy. Bridge ещё не подключён к Codex; write tools fail closed до approval orchestration. Контракт проверен по [MCP tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) и [transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), локальный rmcp 3.2.0.

D10c: RED отсутствующий gateway module. GREEN: scoped context, запрет control token/Origin, запрет обратного доступа к admin API, отзыв при Drop и завершении task, отсутствие credential в durable events. Два regression tests и clippy. Это авторизация transport, MCP forwarding подключается следующим срезом.

D17 acceptance consistency: RED показал accepted task при Artifact.accepted=false. GREEN: задача и артефакт принимаются одной транзакцией; отсутствующий второй artifact откатывает первую запись и event cursor. Полная проверка Rust/UI/spec пройдена до коммита. Исторические workspace с прежним несогласованным флагом требуют отдельной миграции, тест проверяет новые операции.

D18b: RED McpWizard import; дополнительный RED обнаружил, что helper text менял accessible label папки; label исправлен. GREEN failed probe оставляет Enable недоступным, toggle не отправляется, 21 UI test + TS/lint/build. Computer use на реальном daemon: специально созданная `tmp/demo-knowledge` проверена, обнаружены read_text/list_files, role binding «Исследователь» сохранён, переключатель ON и 1 привязка. Визуальная сверка 1600×1000 с reference 03: каталог слева, мастер справа, шаги/инструкция, credential fields/read-only, footer виден; Neo4j не подключён без endpoint/credentials. Browser error/warn log пуст. Инструкция сверена с [Neo4j quickstart](https://neo4j.com/docs/mcp/current/quickstart/) и [configuration](https://neo4j.com/docs/mcp/current/configuration/) 2026-09-08. Реальный вызов через агентский gateway следует отдельно.

D18a: RED `/api/connections/save` 404; GREEN локальная папка проходит save → rejected untested toggle → probe → discovered tools → allowlist → ON → OFF; stale ON получает 409 и доступ остаётся выключенным. Проверено скрытие точных bearer/env secret values из диагностик. 7 daemon tests и clippy. Не является доказательством реального Neo4j или вызова через Codex gateway.

D17b: RED TaskDetail import; GREEN результат не принимается автоматически, action адресован точному taskId, поздний ответ старой карточки не отображается в новой. HTTP RED missing completion → GREEN pinned completion rule в summary. 20 UI tests + 5 daemon tests + TS/lint/build/clippy. Computer use: реальный Codex PID 25408 вернул POLYMAS-UI-42, видны критерий и 1 artifact; нажатие «Принять результат» изменило задачу и запуск на «Принят». После graceful shutdown/restart данные сохранены. Windows build lock устранён запуском отдельной копии dev binary, не отключением проверок.

D17a: RED TaskComposer import; GREEN exact agent target, сохранение idempotency key после неопределённого сетевого исхода, изоляция черновиков адресатов, recursive holon dispatch. 18 UI tests + TS/lint/build. Computer use: один запрос «POLYMAS-UI-42» отправлен через форму реальному Codex; наблюдены runtime.session/turn_started/heartbeat, счётчик running=1 и свечение только нужного узла. Три холона после исправления раскладки отображаются раздельно; активность рёбер не выдумана. Проверка результата/приёмки следует с соответствующей формой.

D16 layout fix: RED stablePositions отсутствует; GREEN перенесённый узел освобождает automatic position, неизменённая группа остаётся на месте, explicit pinned имеет приоритет. Сценарий добавлен после наблюдаемого пересечения групп в computer use.

D15b: RED ArchitectureEditor import; GREEN независимая память без coordinator, CAS revision, C>N/stale draft blocked, 14 UI tests + TS/lint/build. Computer use: созданы и сохранены 3 холона с 6/4/2 участниками и собственными координаторами, ревизия 4, N=12/C=1. Визуально найдена отдельная проблема сохранения автоматических координат после перегруппировки; исправляется следующим срезом.

D16 follow-up: при сверке с `Architecture::permits_message` обнаружено расхождение. RED independent показывал запрещённое ребро; GREEN independent/single не показывают каналы, optional coordinator в network показывает два направления. 5 graph regression tests.

D16b: RED GraphView import; GREEN поиск точного ID среди 5000 определений в DOM-списке без WebGL. TS/lint/11 tests/build. Computer use на реальном приложении: N=12, running=0, WebGL работает, миникарта видна, поиск «Исследователь · 07» показывает одну строку, выбор открывает соответствующий ID в инспекторе; browser error/warn log пуст. Форма архитектуры и взаимодействие с реальным MCP ещё не покрыты этим наблюдением.

D16a: RED отсутствующий graph/model; GREEN 4 сценария: stale/disconnected без свечения, queued без импульса, dedup/expiry доставки, coordinator входит в N / memory отдельно, 5000 уникальных детерминированных координат с сохранением pinned. UI suite 10 tests + TS/lint/build. Это тест определений графа, не 5000 процессов.

Computer use на реальном Vite + daemon приложении, viewport 1600×1000: исходная форма уходила до y=1147, схема была под списком. После исправления предпросмотр и схема видны рядом, footer доступен. Создание роли «Исследователь» и 12 экземпляров через UI завершилось; таблица показывает имена 01–12 и разные ID, счётчики N=12 / running=0. Наблюдение относится к экрану ролей, не к ещё не реализованному WebGL-графу. TS/lint/6 UI tests/build пройдены до коммита; те же проверки запускаются после.
