# 05. Spec-driven development, проверки и выпуск

## 1. Источник истины и процесс изменений

Порядок разрешения противоречий: согласованное требование пользователя → принятая версия продуктовой спецификации → контракт/ADR, согласованный с ней → UX → код → визуальный референс. Изображение не меняет семантику production.

Спецификация 0.1.0 — исходное предложение, подготовленное для ревью, а не свидетельство реализованного приложения. Реализация начинается вертикальными срезами, каждый из которых ссылается на FR, UI, INV и AC.

Для каждого среза:

1. Уточнить требуемое поведение и границы в spec change.
2. Обновить контракт, state transitions и ошибки.
3. Зафиксировать влияющее архитектурное решение ADR.
4. Определить проверяемые acceptance scenarios и fixtures.
5. Реализовать минимальный полный путь UI → backend → реальный исполнитель.
6. Проверить контракт, интеграцию и пользовательский путь.
7. Записать evidence: версии, стенд, команды/сценарии, результаты, известные ограничения.
8. Обновить статус FR и release notes. Не объявлять выполненным требование по одному screenshot.

Новая потребность сначала изменяет spec; несовместимый контракт получает новую major schema/API version. Миграции сопровождаются backup/restore test. Не использовать «TODO» в обязательном контракте принятого среза.

## 2. Структура будущей реализации

    apps/desktop/           UI и Tauri shell
    crates/domain/          сущности, инварианты, state machines
    crates/control/         scheduler, tasks, message router, policies
    crates/worker/          supervisor и lease
    crates/runtime-codex/   exec и отдельный experimental app-server adapter
    crates/mcp-gateway/     transport, bindings, authorization, audit
    crates/storage/         SQLite/PostgreSQL repositories, migrations
    packages/contracts/    генерируемые клиентские типы
    connectors/recipes/    версионируемые инструкции и manifests
    tests/contract/         схемы и adapter fixtures
    tests/integration/      реальные процессы/MCP/БД
    tests/e2e/              пользовательские пути
    tests/load/             синтетические и реальные профили
    specs/                  требования, ADR, evidence

Это план структуры, не уже созданные компоненты.

## 3. Этапы и готовый результат каждого

| Этап | Результат | Блокирующее условие выхода |
|---|---|---|
| M0 · Спецификация | Требования, UX, дизайн, начальные схемы, ADR, список проверок | Нет противоречия N/C/identity; исходный пакет доступен для ревью |
| M1 · Совместимость | Проверенный Codex resolver, auth profile, один процесс, JSONL, resume, sandbox | G-01…G-04; реальный безопасный turn, без подмены simulator |
| M2 · Один агент | Desktop, создание роли/экземпляра, задача, результат, cancel, reopen | AC-01…AC-05, AC-14, AC-20 |
| M3 · Локальная MAS | 10 процессов, очередь, inbox/outbox, ядро on/off, холоны | AC-06…AC-10, AC-16…AC-18 |
| M4 · MCP и знания | Файлы, Neo4j/custom, bindings, provenance, approvals | AC-11…AC-13, AC-19, AC-22 |
| M5 · Production local | Подписанная сборка, backup/restore, ограничения, UI 5000 | Нагрузочный L1/L2, crash tests, security checks; release gate |
| M6 · Распределённая PolyMAS | Worker enrollment, remote artifacts, auth, leases, capacity | AC-23…AC-27; реальные 1000 CLI на разрешённом стенде |
| M7 · Другие исполнители | Claude Code и API адаптеры | Отдельные specs/auth validation и contract suite для каждого |

M0 — текущий объём работы. M1–M7 описывают дальнейшую реализацию. M6 сохраняет исходную цель тысяч одновременно работающих экземпляров. M5 не называется реализацией C=1000.

Календарный срок назначается после M1 и решения о составе команды/канале поставки: заранее обещать срок production без данных о CLI, инфраструктуре и ресурсах некорректно.

## 4. Проверки совместимости до глубокой реализации

| Gate | Вопрос | Проверяемое доказательство / fallback |
|---|---|---|
| G-01 | Работает ли exec/resume на выбранных Windows/CLI? | Два turn с explicit session ID, stream fixtures, Unicode/пути с пробелами; при отказе версия не допускается |
| G-02 | Можно ли безопасно разделять вход между выделенными CLI? | Параллельные 10 процессов, refresh/logout, отсутствие подмены сессий; при отказе поддерживаемые auth profiles или меньший C |
| G-03 | Как реально обеспечить instance filesystem/network policy? | Попытки чтения чужой папки/секретов и write вне root отвергнуты; для hostile workloads OS boundary |
| G-04 | Можно ли исключить скрытый встроенный spawn Codex? | Capability/config probe + task, пытающаяся spawn; если выключить нельзя, дочерние рантаймы выявляются и учитываются либо профиль отклоняется |
| G-05 | Достаточен ли Sigma для группировки и редактирования? | 5000 nodes, 25000 edges, hit test, layout worker, accessibility fallback |
| G-06 | Совместим ли MCP gateway с нужными servers? | Initialize/tools/resources/cancel/timeout/auth, без потери identity; unsupported capabilities обозначены |
| G-07 | Какой реальный process footprint и provider capacity? | Измерения 1/10/50/100, account throttling, projected C=1000 с резервом |
| G-08 | Можно ли допустить app-server в production? | Официальный статус и accepted ADR; иначе остаётся experimental |

Неопределённость G-02/G-03 не решается копированием auth.json или выключением sandbox. Экспериментальная функция не маскируется интерфейсом без предупреждения.

## 5. Приёмочные сценарии

| ID | Условие и действие | Ожидаемый результат |
|---|---|---|
| AC-01 | Чистая область → создать один экземпляр | Ровно один AgentInstance; нет скрытого координатора |
| AC-02 | Отправить локальную задачу агенту | Один выделенный Codex process, связанный attempt/session; JSONL отражён в UI |
| AC-03 | Завершить первый turn и отправить продолжение | Новый процесс принадлежит тому же agentId; resumes только его session ID |
| AC-04 | Сохранить роль v2 во время run на v1 | Текущий run остаётся на v1; следующий выбирает v2 явно |
| AC-05 | Повторить createBatch с тем же ключом | Не появляются дубли; IDs совпадают с исходной операцией |
| AC-06 | N=10, C=10, хватает слотов/квоты | 10 различных PID/start records и sessions одновременно; каждое действие привязано к agentId |
| AC-07 | N=1000, C=10 | Существуют 1000 независимых IDs; максимум 10 активных runtime, остальные в очереди |
| AC-08 | N=1000, C=1000, available=10, start_all | Ни один attempt не запускается; UI сообщает missing 990 slots |
| AC-09 | Включить/выключить координатора и память независимо | Все 4 сочетания валидны; счётчики и dispatch корректны, история сохраняется |
| AC-10 | Холон содержит два дочерних холона, назначить задачу | Вложенные контракты выполняются; ownership cycle отклонён; lineage видна |
| AC-11 | Включить ненастроенный MCP | Открывается мастер; до проверки нет ready/зелёного статуса |
| AC-12 | Подключить read-only Files и запросить запись | Запись отклонена вне LLM; отказ попадает в журнал |
| AC-13 | Подключить Neo4j и отдельно инициализировать память | Видны два независимых шага; нет фиктивных знаний до загрузки |
| AC-14 | Прервать агентный process во время задачи | UI stopping → cancelled/unknown; артефакты сохранены как partial |
| AC-15 | Закрыть и снова открыть UI с продолжающимся daemon | Запуск восстановлен с правильным cursor и состояниями, без второго Codex |
| AC-16 | Дублировать и переупорядочить сообщения transport | Inbox не дублируется, causal links корректны; нет exactly-once claim о внешних эффектах |
| AC-17 | Агент A отправил B; связи C-D не использовались | Светится только факт A→B, C-D остаётся тусклой; доступен eventId |
| AC-18 | Два агента создают бесконечные ответы | Ограничение hops/budget завершает ветку с escalation |
| AC-19 | Выключить required MCP во время run | Новые вызовы blocked немедленно; затронутые задачи показаны |
| AC-20 | Потерять stdout/получить unknown event или crash | Нет ложного успеха; raw safe record, classified error/partial и recovery |
| AC-21 | Включить replay завершённого run | Визуализация не делает ни одного внешнего вызова |
| AC-22 | Экспортировать workspace с credentials | Нет ключей/паролей/refresh tokens; import требует secret remap |
| AC-23 | Worker потерял lease и прежний process жив | Gateway отклоняет старый epoch; новый attempt не повторяет неизвестную запись |
| AC-24 | N=C=1000, ресурсы и provider access предоставлены | 1000 отдельных CLI подтверждены runtime registry и OS; выполняется реальная контрольная задача |
| AC-25 | Нагрузка 5000 узлов / 25000 рёбер | Поиск находит любой ID, граф/список работают в SLO; агрегация не удаляет identity |
| AC-26 | Rate limit общего authProfile | Общая очередь соблюдает cooldown, не создаёт новые аккаунты/процессы для обхода |
| AC-27 | Worker недоступен после внешней write операции | Outcome unknown; reconciliation до нового write |
| AC-28 | Модель объявила «готово», output не прошёл критерии | awaiting_acceptance/needs_review; run не принят автоматически |
| AC-29 | Задача без центрального координатора, N=1 | Прямой Codex, единственный session/turn; инфраструктура не добавляет LLM |
| AC-30 | Изменить аргументы уже разрешённой операции | Старое approval не действует; exact hash проверяется |
| AC-31 | Отозвать source binding после чтения | Новые чтения запрещены; UI объясняет retained context и предлагает новую session |
| AC-32 | Два editor изменяют revision (командный режим) | Второй получает revision conflict; доступен diff, нет silent overwrite |
| AC-33 | Потеря диска/полный диск во время finalizing | Новые launch остановлены; успех не объявлен без сохранения |
| AC-34 | Сработало scaling rule на очереди | Новые ID создаются в min/max/cooldown; правила видны в audit; активный агент не архивируется |
| AC-35 | Узел running потерял heartbeat | После grace смена на stale/offline, свечение прекращено |
| AC-36 | Выключить анимацию/использовать только клавиатуру | Все действия доступны через список, таблицу и focus navigation |

## 6. Матрица трассировки

| Требования | UI | Компоненты | Приёмка |
|---|---|---|---|
| FR-01, FR-16 | 01, 13 | configuration, storage, export | 05, 15, 22, 32 |
| FR-02, FR-03 | 05, 06 | roles, instances | 01, 03, 04, 05 |
| FR-04 | 04, 11 | scheduler, scaling, admission | 06, 07, 08, 24, 26, 34 |
| FR-05, FR-06 | 03, 04 | holons, topology, dispatch | 09, 10, 29 |
| FR-07, FR-08 | 06, 07 | task state, runtime adapter, acceptance | 02, 03, 28, 29 |
| FR-09 | 03, 06, 10 | router, inbox/outbox | 16, 17, 18 |
| FR-10 | 08, 12 | MCP manager, policy | 11, 12, 19, 30 |
| FR-11 | 09 | sources, knowledge, provenance | 13, 31 |
| FR-12 | 03, 10 | event log, graph projections | 17, 20, 21, 25, 35, 36 |
| FR-13 | 06, 07, 12 | supervisor, leases, cancel | 14, 15, 23, 27 |
| FR-14 | 07, 10 | artifacts, acceptance | 14, 28, 33 |
| FR-15 | 02, 11 | quotas, metrics, usage | 08, 24, 26 |
| FR-17 | 01, 11 | runtime registry | G-01…04, отдельные adapter suites |
| FR-18 | 11, 12, 13 | auth, backups, release | 22, 23, 30, 32, 33 |

## 7. Нефункциональные критерии

Числа ниже — цели, которые предстоит подтвердить. Они не являются измерениями этого пакета.

| ID | Цель | Метод измерения |
|---|---|---|
| NFR-01 | Открытие локальной области с 1000 агентами ≤ 3 s p95 | 30 запусков, cold/warm раздельно, без сетевой модели |
| NFR-02 | Реакция UI на выбор узла ≤ 100 ms p95 | 5000 nodes/25000 edges на стенде L1 |
| NFR-03 | Граф ≥ 30 fps при pan/zoom | L1, 60 s трасса; reduced-motion и список остаются рабочими |
| NFR-04 | Durable event → UI ≤ 500 ms p95 local, ≤ 2 s remote | 1000 events/s sustained, payload median 1 KiB |
| NFR-05 | Admission служебной задачи ≤ 200 ms p95 при свободном слоте | Без provider startup/generation latency |
| NFR-06 | Подтверждение cancel ≤ 500 ms local | Фактический process stop: grace до 10 s + force; внешняя операция отдельно |
| NFR-07 | Ноль потерянных подтверждённых сообщений при crash/restart | Fault injection outbox/inbox transaction boundaries |
| NFR-08 | Восстановление local daemon ≤ 30 s после старта | Согласованный snapshot + журнал; provider reconnect отдельно |
| NFR-09 | Утечка памяти < 10% после стабилизации за 8 h soak | Фиксированные N/C и churn workload; исключить полезный рост retention |
| NFR-10 | Backup restore локального проекта ≤ 15 min на тестовом наборе | 1 GiB artifacts + 100000 events; проверка hash |
| NFR-11 | Командный control plane 99.9% за месяц | Только после HA-поставки M6; planned maintenance учитывается по SLO |
| NFR-12 | Контраст и клавиатурные сценарии | WCAG 2.2 AA применимые критерии, ручная проверка screen reader |

Стенд L1: Windows 11 x64, 8 логических CPU, 16 GiB RAM, SSD, аппаратно ускоренный WebView2, окно 1600×1000, 100% и 200% scaling. Конкретные CPU/GPU/driver/OS build записываются в evidence; результаты не переносятся автоматически на слабые машины.

L2: local runtime bench с 1/10 CLI, затем отдельно 50/100 на оборудовании, где preflight подтверждает память/слоты. L3: cluster на 1000 выделенных CLI с заранее разрешёнными credentials, квотами и ресурсами. Все endpoints и секреты тестового стенда изолированы от production.

## 8. Нагрузочные профили

1. **UI simulation:** 5000 instances, 25000 edges, 1000 events/s 15 min; burst 10000/s 10 s. Проверяет UI/backpressure, не Codex.
2. **Messaging:** 100000 уникальных сообщений, 10% дубликатов, 5% out-of-order; ожидается уникальный inbox и отсутствие утраты committed receipts.
3. **Process churn:** 1000 последовательных attempts на 10 slots; отсутствие orphan PID/дескрипторов, корректные role/session bindings.
4. **Real MAS:** 10 CLI обрабатывают разные задания в разных папках; сообщение, память, результат и cancel проверяются по OS/runtime logs.
5. **Real PolyMAS:** 1000 процессов одновременно выполняют контрольные короткие задачи и обмены, затем 8 h soak на разрешённом режиме. Реальное количество активных процессов фиксируется непрерывно, а не только число созданных IDs.
6. **Failure:** kill worker/control, timeout MCP, token expiry, revoked auth, disk full, network partition, duplicated lease, delayed approval, malformed UTF-8/JSONL.

Реальные provider-запуски не выполняются бесконтрольно из обычного CI. Контрактные fixtures и имитаторы отдельно помечаются; платные/подписочные integration jobs имеют выделенный бюджет.

## 9. Качество и тестовая стратегия

Domain tests — инварианты и переходы; property tests — ownership DAG, quotas, count preservation, message dedup; contract tests — схемы/CLI fixtures/MCP capabilities; integration — реальные process tree, SQLite/DB transactions и transports; e2e — критические пользовательские пути.

Визуальная проверка покрывает 3 референсных экрана, пустое состояние, 1/10/1000, ядро on/off, ошибка MCP, stale telemetry, 200% scaling и reduced motion. Pixel snapshot дополняет поведенческую проверку, не заменяет её.

Security проверки: secret leakage в экспорт/лог, prompt injection попытка расширить scope, path traversal/junction, cross-agent read/write, cross-tenant access, stale lease, arbitrary shell import, unsafe markdown.

Обновление Codex: pinned version → schema diff → adapter fixtures → real smoke → canary → rollout; при regression runtime version возвращается на поддерживаемую. Сессию новой версии не открывают старым CLI без проверки совместимости.

## 10. Production gate

Выпуск разрешён при выполнении обязательных AC текущего этапа, отсутствии блокирующих ошибок, проверенном installer/signing, SBOM/license inventory, backup/restore, rollback, диагностике и пользовательских инструкциях.

Каждый release содержит support matrix: OS, CLI versions, auth methods, tested N/C, MCP recipes, ограничения изоляции, ресурсы стенда, известные ограничения.

Smoke на локальной подписке не является доказательством права раздавать подписочный доступ внешним пользователям через hosted service. Для командной поставки проверяются актуальные условия выбранного провайдера и схема авторизации; не закладывается несуществующая лицензия на перепродажу.

Откат: остановить выдачу новых задач, drain, создать согласованный backup, переключить совместимую сборку, проверить миграции и session compatibility, возобновить. Разрушительный downgrade БД без backup запрещён.

## 11. Доказательства для сдачи этапа

Для каждой AC: дата, commit/build, spec version, environment, inputs, expected/actual, журналы без секретов, PASS/FAIL/SKIPPED, ссылка на issue для отклонения.

PASS ставится только по фактически выполненной проверке. Текущий пакет содержит проверку документов/макета/схем; проверки реального многопроцессного исполнения ещё не проводились.
