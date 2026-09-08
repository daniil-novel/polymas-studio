# PolyMAS Studio — реализация

Основание: спецификация 0.1.0 и запрос владельца от 08.09.2026. Каждый пункт — отдельный небольшой срез, с regression test до реализации, self-review, проверкой до и после коммита. Статус `[x]` означает реализованный срез, а не автоматически пройденный production gate.

## Очередь работ

- [x] D01. Зафиксировать спецификацию, референсы и план в Git.
- [x] D02. Настроить Rust/TypeScript workspace, проверки и CI.
- [x] D03. Версионируемые роли и независимые экземпляры; admission N/C проверяется в D06.
- [x] D04. Транзакционное SQLite-хранилище, ревизии, миграции и события.
- [x] D05. Модель архитектуры: холоны, связи, координатор и память; формы в D14–D19.
- [x] D06. Очередь задач, зависимости, admission, lease и правила приёмки; durable service в D13.
- [x] D07. Codex resolver, JSONL, отдельный процесс, явный resume и отмена.
  - [x] D07a. Неизменяемый владелец всех прошлых сессий, migration и runtime enforcement; отвергнутое событие не освобождает экземпляр до reap.
  - [ ] D07b. Квалифицировать отдельный интерактивный Codex adapter для MCP write approvals: точное решение по thread/turn/item/args, deny для остальных действий, без blanket approval. Exec 0.144.1 отменил destructive MCP call после owner decision; реальная запись пока не прошла gate.
    - Предварительная проверка 0.144.1 и остающиеся условия: [app-server qualification](specs/evidence/codex-app-server-01441.md).
- [x] D08. Надёжная маршрутизация сообщений и защита от циклов; gateway связывается в D10.
- [x] D09. MCP-рецепты, stdio/HTTP, диагностика и секретное хранилище; расширенные integration tests в D22.
- [ ] D10. Gateway: bindings, ограничения инструментов и approvals.
  - [x] D10a. Durable exact-operation approvals; привязка к gateway следует отдельно.
  - [x] D10b. Role/agent bindings, tool allowlist и пересечение namespaces.
  - [x] D10c. Отдельная авторизация gateway и отзыв attempt credentials; forwarding и подключение Codex следуют отдельно.
  - [x] D10d. Обнаружение инструментов и read forwarding; STDIO bridge, runtime wiring и write approvals далее.
  - [x] D10e. Настоящий STDIO bridge на rmcp; подключение к execution pipeline далее.
  - [x] D10f. Runtime выдаёт отдельный gateway grant и отзывает его после попытки; actual Codex MCP smoke и write approvals далее.
  - [x] D10g. Peer discovery, send/inbox/ack через gateway; task delegation, wait/resume и write operation UI далее.
  - [x] D10h. Scheduler park/wake и turn budget; gateway yield и durable автоматическое пробуждение далее.
  - [x] D10i. Durable gateway wait directive с проверкой срока/адресата/бюджета; runtime park после reap далее.
  - [x] D10j. Runtime park/wake и task-scoped resume; UI и actual Codex wait smoke далее.
  - [x] D10k. UI ожидания и actual Codex message wake/resume C=1; deadline smoke отдельно.
  - [x] D10l. Exact operation дополнительно включает run, namespace и resource scope hash; старые unscoped записи читаются, но не дают разрешения. Approval gateway/ожидание/UI далее.
  - [x] D10m. Одна durable operation request на attempt: повтор MCP возвращает исходный ID/решение/expiry, другая операция не заменяет ожидающую. Gateway/runtime/UI далее.
  - [x] D10n. Scheduler waiting_user: освобождение lease после reap, persisted approval ID/origin epoch, точное продолжение и отмена/turn budget/first-accepted. Проверка исхода операции и runtime wiring далее.
  - [x] D10o. Резервирование epoch продолжения без process slot; порядок задач экземпляра, сохранение после restart и необратимое погашение при отмене/crash. [ADR-017](specs/adr/017-approval-continuation.md); atomic approval wiring далее.
  - [x] D10p. Fingerprint включает originEpoch и отдельный future epoch; receipt ищется по исходному turn, legacy без origin fail closed. Atomic request в Engine далее.
  - [x] D10q. Engine атомарно сохраняет reservation/approval/receipt; сам задаёт identity, future epoch, connection revision и resource hash, проверяет live∩pinned write scope. Runtime/gateway/UI далее.
  - [x] D10r. Durable owner decision/CAS retry, park/restart/wake/expiry и одноразовое consume по execution index; повторная проверка области и текущего lease. HTTP/runtime/gateway/UI далее.
  - [x] D10s. Runtime после успешного reap сохраняет partial и waiting_user; решение/expiry будит reserved turn той же task session, контекст содержит approval ID. Проверено дочерним fixture process; owner API/UI и gateway tools далее.
  - [x] D10t. Потерянный executing lease превращает внешний исход в unknown; до явной сверки запрещены retry/accept задачи. Sweep использует индекс состояния и metadata; owner reconciliation в core, его UI/API далее.
  - [x] D10u. Debug approval/operation не раскрывает аргументы и ресурс; regression проверяет вложенное содержимое без изменения сохраняемой операции.
  - [x] D10v. Gateway request/status: служба задаёт identity и резерв, 15-минутный срок, retry без продления, статус только origin/точному continuation; взаимоисключение с message wait. Исполнение внешнего вызова следующим срезом.
  - [x] D10w. Execute по одному approvalId: frozen args, проверка scope/credentials до consume, атомарный результат, без повторной отправки; cancel/revoke/partial error → unknown. Реальные STDIO/HTTP fixtures; actual Codex write и результат операции в owner UI далее.
  - [x] D10x. Неиспользованные pending/approved автоматически отзываются при закрытии origin/continuation, отмене и recovery; активный запрос/очередь/исполнение сохраняются. UI показывает «Отозвано», история owner decision не стирается.
  - [x] D10y. Чтение сохранённого результата страницами UTF-8 с SHA-256 и bounded inline; проверены Unicode, escaping, границы и reopen. Подключение к gateway/owner API/UI следующим срезом.
  - [x] D10z. Короткие execute/status receipts и scoped read_operation_result; owner result pages/metadata доступны после завершения task. Реальный большой STDIO reply собран без второй записи; UI просмотра далее.
  - [x] D18c. Owner approvals API: страницы metadata, frozen target/точные аргументы, доступность решения, CAS/повтор; agent credential не допускается. UI и MCP инструменты далее.
  - [x] D18d. Экран подтверждений: фильтры/страницы, точные параметры, решение без подмены, guard при late response/disconnect; waiting_user в задачах/графе/replay. CU fixture 1600×1000 и 1280×800, отказ сохранён. Agent request/execute инструменты и actual Codex write smoke далее.
  - [x] D18e. Owner API/форма сверки unknown с проверенным исходом и заметкой, CAS/idempotency, история результата; черновик переживает обновление карточки. CU отдельного fixture, без внешнего вызова.
  - [x] D18f. Фоновое обновление сохраняет DOM раскрытой операции; до свежего ответа и при ошибке решения заблокированы. Явный refresh/uncertain POST по-прежнему сбрасывает старую actionable карточку.
  - [x] D18g. Читаемая одноколоночная карточка при узком окне; browser regression 736/1280/1600px и отдельный CI browser job. Остальные экраны/полный visual gate D23 продолжаются отдельно.
  - [x] D18h. Просмотр saved MCP response по частям и JSON export с SHA-256; отмена/смена карточки, corrupted reply и read retry. CU настоящего daemon fixture; Chromium download проверен, native save gate остаётся открытым.
- [x] D11. Источники, утверждения, связи и происхождение знаний; UI/gateway следуют отдельно.
- [ ] D12. Артефакты с hash, экспорт, backup и восстановление.
  - [x] D12a. Content-addressed артефакты, provenance и безопасный импорт.
  - [x] D12b. Backup/restore snapshot + blobs; переносимый экспорт конфигурации следует с workspace service.
- [ ] D13. Фоновая служба и защищённое desktop-подключение.
  - [x] D13a. Workspace service, команды конфигурации и durable history.
  - [x] D13b. Durable queue, снимки запуска, recovery и ручная сверка исхода.
  - [x] D13c. Авторизованный локальный transport, Origin allowlist, bounded requests и replay API.
  - [x] D13d. Execution pipeline: процесс, событие, inbox receipt, session и artifact; daemon entrypoint далее.
  - [x] D13e. Daemon entrypoint и управление задачами; два реальных Codex turn через HTTP проверены.
- [ ] D14. Desktop shell, навигация и настоящие сохранённые данные.
  - [x] D14a. React shell с реальным HTTP API; Tauri launcher и native IPC следуют отдельно.
  - [x] D14b. Проверенный native HTTP client для IPC; запуск службы и Tauri окно далее.
  - [x] D14c. Launcher отдельного daemon; закрытие handle сохраняет работу, явный shutdown выполняет drain. Persistent reconnect и Tauri далее.
  - [x] D14d. Сериализуемая запись службы и reconnect через vault с проверкой workspaceId; хранение профиля и Tauri окно далее.
  - [x] D14e. Startup adoption и защита от потерянного launcher; durable profile и Tauri далее.
  - [x] D14f. Durable native profile, startup lock и reconnect без нового процесса; Tauri окно и OS vault smoke далее.
  - [x] D14g. Сохраняемая политика закрытия с reset; применение в Tauri и диалог далее.
  - [x] D14h. Frontend lifecycle dialog и reconnect для native startup; Tauri commands и visual native verification далее.
  - [x] D14i. Tauri shell, IPC ACL, tray, single instance, staging/build; standalone process и real OS vault smoke. Native visual/close-through-window gate остаётся открытым из-за недоступного Windows CU helper; capacity/workspace/settings и installer drain далее.
- [ ] D15. Формы ролей и массового создания экземпляров.
  - [x] D15b. Форма архитектуры и холонов с сохранением через ревизии; ресурсные scaling rules далее в D20.
  - [x] D15a. Роль, новая версия, массовое создание, список экземпляров и stale-form guard; расширенные действия экземпляров далее.
- [ ] D16. WebGL-граф, поиск, layout worker и доступный список.
  - [x] D16a. Проекция состояний/связей и bounded deterministic layout; renderer следует отдельно.
  - [x] D16b. Sigma renderer, worker, миникарта, фильтры и виртуальный список; drag/pinning, агрегация холонов и расширенная клавиатурная навигация далее.
  - [x] D16c. Размеры групп учитываются в раскладке, центр резервируется памяти; изменение состава сбрасывает несовместимые auto positions, manual pins сохраняются. Пересечения длинных подписей исправляются отдельно.
  - [x] D16d. Подписи измеряются перед рисованием и исключают пересечения; длинные имена сокращаются по grapheme без изменения identity, полный текст в инспекторе/списке.
  - [x] D16e. Новое live-событие немедленно использует текущие часы; граф, счётчик и инспектор согласуют heartbeat/stale. Устаревший heartbeat не отключает остановку задачи.
  - [x] D16f. Отдельная команда позиции узла с revision/history, проверкой координат/существования и снятием закрепления. Перетаскивание в UI следует отдельно.
  - [x] D16g. Drag/pinning и снятие закрепления в графе/списке; Escape/blur отменяют жест, conflict требует чтения свежего состояния. Chromium regression и CU настоящей службы; touch/массовое редактирование/LOD остаются далее.
  - [x] D16h. Навигация по холонам/потомкам, пересечение с поиском, возврат ко всем агентам и открытие задачи выбранному холону; масштаб камеры и свёрнутые группы далее.
  - [x] D16i. Выбранный холон автоматически вписывается в окно; fit учитывает видимые узлы, ждёт layout worker и не меняет сохранённые координаты.
- [ ] D17. Задачи, чат, результаты, управление выполнением.
  - [x] D17a. Composer и прямой запрос агенту, адресация/критерии/stop policy; результаты и приёмка в UI далее.
  - [x] D17b. Список последних запусков, результат, приёмка и действия; полная пагинация истории/вложения/редактор DAG далее.
  - [x] D17c. Общая остановка доступна при очереди/ожидании сообщения/решения даже на паузе; проверяется актуальная сводка, а не только запущенные процессы.
  - [x] D17d. Owner API полной истории: индексированные страницы запусков/задач, Unicode-поиск, фильтры и курсор по времени/ID; payload содержит только сводку. Подключение экрана следующим срезом.
  - [x] D17e. Экран полной истории с независимыми страницами/фильтрами, прямым открытием запуска и сохранением результата/черновика при background refresh; скрытый экран не опрашивает историю. Browser/CU на read-only authored fixture; DAG/вложения/другие D17 требования остаются.
  - [x] D17f. Карточка задачи читается из durable storage, даже если старая запись отсутствует в рабочем наборе scheduler; чтение результата остаётся owner-only.
- [ ] D18. MCP-мастер, bindings, диагностика и approvals в UI.
  - [x] D18a. HTTP lifecycle подключений и protected secret commands; UI/gateway далее.
  - [x] D18b. Каталог/мастер и bindings в UI; atomic activation, OAuth, удаление/ротация и approvals UI далее.
- [ ] D19. Знания, журнал, replay и история конфигурации в UI.
  - [x] D19a. Подготовка знаний к редактору: atomic idempotent commands, CAS проверки, ограниченные страницы и поиск; HTTP/UI/gateway далее.
  - [x] D19b. Owner API знаний и карточка происхождения; отсутствие общей памяти не скрывает сохранённые данные владельца. UI/gateway далее.
  - [x] D19c. Трёхпанельный редактор источников/утверждений/provenance, проверка, связи и страницы; CU на настоящем daemon. Knowledge graph/indexing/gateway и replay далее.
  - [x] D19d. Storage projection для агентского чтения: обязательный namespace, конкретный connection и проверка обеих сторон связи до limit; gateway далее.
  - [x] D19e. Scoped read-only knowledge gateway, отзыв памяти/bindings и provenance; публикация агентом, graph pulses и actual Codex smoke далее.
  - [x] D19f. Observable memory pulse в агентном графе по событиям чтения; actual provider/CU smoke далее.
  - [x] D19g. Actual Codex search + provenance read и приёмка fixture через UI; первый конфликт контракта сохранён как failed acceptance. Визуальный импульс/replay и внешний Neo4j остаются отдельными gate.
  - [x] D19h. Read-only recording API: pinned configuration и ограниченные страницы событий до frozen cursor; UI replay далее.
  - [x] D19i. Отдельный read-only replay: шкала/скорость/пауза, записанные состояния и импульсы, поиск/фильтры, body события по запросу; CU реального memory run. Серверные checkpoints для >100 000 событий и масштабный gate далее.
- [ ] D20. Масштабирование по правилам и ресурсные ограничения.
  - [x] D20a. Сводка активности по каждому экземпляру отдельно от последних 1000 задач; старый running не исчезает с графа и остаётся доступным для остановки. Индексы/нагрузочные gate далее.
- [ ] D21. Удалённые workers, enrollment, fencing и кластерный storage.
- [ ] D22. Реальные Codex smoke/resume/concurrency и failure tests.
- [ ] D23. Browser e2e, computer use и визуальная сверка трёх экранов.
- [ ] D24. Упаковка Windows, SBOM, release workflow и инструкции.
  - [x] D24a. Переносимая нормализация CycloneDX 1.5 и сводка лицензий с регрессионными проверками в CI; настоящая NSIS-сборка 9aa7b56 и четыре schema-valid SBOM. [Доказательства и команды](specs/evidence/windows-packaging-9aa7b56.md). Release workflow, notices, подпись и installer lifecycle далее.
- [ ] D25. Финальный self-review, README, changelog и матрица приёмки.
  - [x] D25a. README текущего исполняемого среза: сборка, сценарии, данные, проверки, диагностика и явные незакрытые требования; финальный release review далее.

## Внешние условия для выпуска

- GitHub: приватный `daniil-novel/polymas-studio` создан, privacy проверена, история отправлена через сохранённый вход Git.
- Подписанный installer: нужен сертификат владельца/канал подписи; ключи не включаются в репозиторий.
- Реальный C=1000 и 8-часовой soak требуют доступного стенда, квоты и измерений. Simulator не подтверждает AC-24.
- Реальный Neo4j требует доступного endpoint и credentials; connector protocol tests не заменяют эту проверку.
- Claude/API остаются будущими адаптерами по FR-17; первый исполняющий backend — Codex.

## Правила доказательств

Не снимать требования ради зелёного отчёта. Обозначать отдельно IMPLEMENTED, VERIFIED, BLOCKED и NOT IMPLEMENTED. Фиксировать конкретный тест, результат и ограничения в `specs/evidence/`. Не объявлять выпуск production-ready, пока обязательные gate не пройдены.
