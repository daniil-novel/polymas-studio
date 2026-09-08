# 03. Архитектура и контракты исполнения

Архитектурный baseline для реализации. Здесь «должен» означает требование продукта; наличие конкретной возможности внешнего исполнителя подтверждается отдельно.

## 1. Выбранный стек

| Слой | Решение | Причина и граница |
|---|---|---|
| Desktop shell | Tauri 2, Windows 11 x64 | Нативная упаковка, OS-интеграции, ограниченный IPC |
| UI | React + TypeScript, Vite | Компоненты сложного редактора, типизированные контракты |
| Граф | Sigma.js + Graphology, собственные слои состояний | WebGL для многих узлов; редактор форм и архитектуры остаётся DOM |
| Раскладка | Отдельный Web Worker | Расчёт не блокирует ввод; координаты сохраняются |
| Control/worker | Rust + Tokio | Процессы, ограниченные очереди, typed IPC, общий код daemon и worker |
| Локальные данные | SQLite, WAL, один логический writer | Транзакции конфигурации, задач и журнала; файл на локальном диске |
| Файлы | Content-addressed artifact store | Проверка hash, версионирование и перенос |
| Секреты | OS credential store; отдельный vault для managed server | Ссылки на секреты в конфигурации |
| Командная/cluster версия | Rust services, PostgreSQL, object storage, TLS worker API | Отдельная topology без общей SQLite по сети |
| Контракты | JSON Schema + сгенерированные TS/Rust модели | Независимость UI, адаптеров и тестов |

Версии зависимостей фиксируются lockfiles в фазе реализации. «Последняя версия» не является контрактом. Sigma.js выбран для рендеринга графа на WebGL по его официальной документации; сложные интеракции холонов должны пройти spike до окончательной фиксации. [Sigma.js](https://www.sigmajs.org/docs/)

Веб-клиент не получает shell или полный Tauri API. Для каждого окна/команды задаются разрешения и отдельная проверка аргументов на стороне Rust. Tauri capabilities — один уровень ограничения интерфейса, а не песочница для Codex. [Tauri capabilities](https://v2.tauri.app/security/capabilities/)

## 2. Компоненты

Схема исполнения:

    Desktop UI
        │ типизированные команды / события
    Tauri bridge
        │ защищённый локальный IPC
    PolyMAS daemon
        ├── Configuration + revisions
        ├── Task planner + dispatcher
        ├── Scheduler + quotas + leases
        ├── Message router + inbox/outbox
        ├── Policy engine + approvals
        ├── Runtime adapter registry
        ├── Event journal + projections + replay
        ├── MCP gateway + connection manager
        └── Artifacts + knowledge metadata
                 │
             Worker supervisor
                 ├── Codex process for agent A
                 ├── Codex process for agent B
                 └── Codex process for agent N

Каждый Codex работает через собственный адаптерный контекст: runtimeProcessId, agentId, sessionId, attemptId, working directory, leaseEpoch. MCP gateway не становится общим «мозгом»; он проверяет и журналирует операции.

Daemon живёт отдельно от окна. Закрытие UI по умолчанию спрашивает один раз настройку «Оставлять задачи в фоне / останавливать». При фоне остаётся значок tray; после перезапуска UI подключается к тому же daemon. Удаление приложения сначала переводит daemon/worker в drain.

Локальный транспорт: named pipe с ACL текущего пользователя на Windows, domain socket с правами владельца на Unix. Не открывать публичный порт для локального режима. Командный сервер получает отдельный аутентифицированный HTTPS/WebSocket интерфейс; браузер не подключается к Codex напрямую.

## 3. Codex: два адаптера

### 3.1. CodexExecAdapter — исходный production-кандидат

Внешний процесс запускается через OS spawn с массивом аргументов, без shell-интерполяции. Найденный wrapper .ps1 не выполняется через собранную строку: resolver определяет проверенный реальный бинарник/entrypoint, записывает путь и версию.

Концептуальный первый запуск:

    codex exec --json --color never --sandbox read-only --cd <agent-workspace> -

Инструкция поступает в stdin. Права записи добавляются только по профилю workspace-write. Параметры конфигурации компилируются проверенным TOML-сериализатором. При использовании папки без Git явно применяется поддерживаемый skip-git-repo-check; наличие Git не считается безопасностью.

Фактический набор флагов проверяется для версии CLI. В частности, у установленной 0.144.1 справка resume не содержит все флаги первого exec: адаптер не переносит argv вслепую. cwd повторного процесса задаётся OS spawn; обязательная политика проверяется отдельно.

Первый thread.started возвращает providerSessionId, который атомарно связывается с agentId. Следующие turn используют **конкретный** session ID:

    codex exec resume --json <provider-session-id> -

Опция --last в продукте запрещена: при сотнях сессий она неоднозначна. Один session ID защищён single-writer lock.

JSONL stdout разбирается инкрементально; stderr направляется в ограниченный диагностический канал. Парсер обрабатывает частичные строки, UTF-8 на границе chunk, неизвестные поля, max frame size, завершение без terminal event.

Нормализуются thread.started, turn.started, item.started/updated/completed, turn.completed, turn.failed и error. События не обязаны содержать каждый токен ответа. UI не показывает посимвольный streaming, если CLI его не отдаёт. [Codex non-interactive](https://developers.openai.com/codex/noninteractive)

Успешный process exit сам по себе не подтверждает полезный результат. Требуются терминальное событие или корректно классифицированное завершение, сохранённый output и проверка результата. Если результат есть, но процесс завершился ошибкой, run получает partial/needs_review по политике.

Capabilities исходного адаптера:

| Capability | Поведение |
|---|---|
| separate_process | Да, один Codex на работающий экземпляр |
| explicit_session_resume | Да, после интеграционной проверки конкретной версии |
| structured_events | Да, наблюдаемые JSONL-события |
| streaming_text | Только при фактическом наличии delta; не обязательна |
| steer_active_turn | Нет; очередь следующего turn или interrupt |
| interactive_cli_approvals | Нет; не эмулировать stdin-ответы на TUI |
| platform_tool_approvals | Да, через PolyMAS gateway/operation workflow |
| hard_pause | Нет; пауза выдачи и interrupt |
| usage | Поля, фактически переданные CLI; остальные null |
| context_export/import | Не обещается; session resume локально, remote migration отдельно |

Продукт не использует dangerously-bypass-approvals-and-sandbox. Если политика действия не может быть обеспечена этим адаптером, действие недоступно до выбора подходящего изолированного профиля или другого адаптера.

### 3.2. CodexAppServerAdapter — отдельный экспериментальный профиль

Этот интерфейс подходит для встраиваемых клиентов и имеет thread/turn, события и запросы решений. Однако документация, полученная 08.09.2026, прямо обозначает app-server и WebSocket transport как экспериментальные и не поддерживаемые для production workloads. Использование не скрывается под production-кнопкой. [Codex App Server](https://developers.openai.com/codex/app-server)

Для spike: отдельный процесс app-server на каждый AgentInstance, локальный stdio, initialize → initialized → thread/start либо thread/resume → turn/start. Возможности turn/steer, turn/interrupt, approvals включаются только по схеме выбранной версии. Нельзя помещать 1000 агентов как 1000 threads одного процесса и выдавать это за 1000 Codex CLI.

Схемы генерируются самим выбранным бинарником через generate-json-schema/generate-ts и сравниваются при обновлении. Протокол app-server имеет собственный формат JSON-RPC-подобных сообщений; нормализованные PolyMAS events не копируют его как публичный API продукта.

Допуск в production требует изменения официального статуса поддержки или отдельного формального решения о принятии риска с поддерживаемой версией, тестами совместимости и rollback. До этого основной выпуск должен выполнять заявленные сценарии на exec с указанными ограничениями.

### 3.3. Общий контракт RuntimeAdapter

Операции: probe, validateProfile, startAttempt, resumeAttempt, sendNextTurn, interrupt, health, drain, collectArtifacts, readUsage, dispose.

Каждая возвращает результат или типизированную ошибку: unsupported_capability, invalid_config, authentication_required, rate_limited, provider_unavailable, sandbox_denied, protocol_error, process_crashed, timeout, cancelled, unknown_external_effect.

startAttempt принимает instance snapshot, task snapshot, resolved resources, secret handles, output schema и lease token. Адаптер не принимает произвольные команды из renderer.

Event stream выдаёт нормализованные события с сохранённым source_type и ссылкой на очищенное исходное событие. Unknown events сохраняются как runtime.unknown_event и не переводят состояние в успех.

## 4. Идентичность, роли и авторизация

agentId стабилен между процессами и независим от имени/номера. runtimeProcessId — UUID записи процесса, а OS PID хранится отдельно вместе с start time и worker bootId, чтобы избежать ошибок при повторном использовании PID.

Роль фиксируется как roleId + version + contentHash. В exec она компилируется в документированную инструкцию задачи/контекста. Не заявляется управление настоящим system message модели. Для доступа к shell/MCP ограничение задаётся политикой исполнения, а не текстом роли.

AuthProfile принадлежит аккаунту и host. Credentials предоставляет штатный механизм Codex. Нельзя размножать auth.json по папкам экземпляров или отправлять refresh token на удалённые hosts.

Для локальной группы агентов возможен общий app-owned CODEX_HOME авторизационного профиля при собственных workspace и session IDs. Auth/login/logout/refresh-sensitive операции сериализуются в credential manager; профиль не переписывается на каждом turn. Совместная работа нескольких CLI с этим профилем — обязательный smoke/spike G-02 до выпуска.

Если Codex самостоятельно обновляет токены вне контроля manager, совместимость его refresh/cache при параллельных процессах должна быть доказана. При отказах используется поддерживаемое разделение profiles/host authentication; самодельный протокол обмена OAuth-токенами не вводится.

Подключение существующего пользовательского Codex home требует явного выбора в мастере и проверки effective config. По умолчанию PolyMAS не меняет глобальный config.toml пользователя. Управляемые параметры передаются snapshot-конфигурацией; унаследованные внешние MCP не получают доступ автоматически.

CODEX_HOME и рабочая папка не заменяют OS sandbox. Для hostile или межпользовательских workloads нужен отдельный OS principal/container/VM, недоступные агенту credentials и файловые границы.

## 5. Машины состояний

### AgentInstance

configured → idle → queued → starting → running → idle.

Во время running возможны tool_wait, waiting_message, waiting_user, rate_limited. stopping → stopped; failed может вернуться queued только новой попыткой. archived исключён из dispatch.

Instance state — проекция runtime + текущей задачи. История завершённых задач не перезаписывается статусом следующей.

### Task / Run

draft → validating → queued → running → awaiting_acceptance → succeeded.

Дополнительно: blocked, paused, cancelling, cancelled, partial, failed. paused не обещает заморозку OS-процесса: это запрет новых turn/подзадач. cancel является идемпотентной командой.

### Attempt

reserved → launching → active → finalizing → succeeded/failed/cancelled/unknown.

Терминальное состояние неизменно; повтор — новый attempt ID. При process crash с неизвестным результатом внешнего действия запрещён blind retry.

### MCP connection

disabled → configuring → testing → ready → degraded/auth_required/offline.

enabledDesired хранится независимо от health. Отзыв binding действует немедленно, даже если старая capability ещё отображается в каталоге текущего Codex turn.

## 6. Планировщик и admission

Планировщик выдаёт одну активную попытку на агента и применяет одновременно:

- workspace/holon/run concurrency;
- worker capacity и reserve памяти;
- authProfile/provider concurrency;
- модельные/сервисные rate limits, если известны;
- бюджеты задачи, число turn, TTL и deadline;
- required MCP availability и права.

Дисциплина: weighted fair queues между run/holon, priority с aging, запрет starvation. Приоритет не отменяет бюджеты. Задача, ожидающая сообщение/человека, не должна постоянно держать модельный слот; в exec turn завершается структурированным ожиданием, потом продолжается новым процессом.

Стратегии admission:

- queue_allowed: регистрируются все задания, выдаются в пределах слотов;
- start_all: резервируются все нужные слоты либо запуск не начинается; reservation имеет TTL.

Лимиты аккаунта общие. При rate_limited новые attempts профиля приостанавливаются, Retry-After учитывается если доступен; иначе exponential backoff с jitter, верхним пределом и max retries. Отсутствующие сведения о квоте не считаются разрешением на 1000 запросов.

Stop workspace атомарно увеличивает generation, отменяет очереди и запрещает lease старого generation. Worker перед новым turn/tool сверяет generation и срок lease.

## 7. Доставка сообщений

Агентам доступен собственный платформенный MCP с инструментами PolyMAS, которые **будут реализованы приложением**, а не существуют в Codex:

| Инструмент | Семантика |
|---|---|
| polymas.send_message | Записать сообщение разрешённому адресату; вернуть durable receipt |
| polymas.list_inbox | Получить разрешённые сообщения с cursor |
| polymas.ack_message | Явно подтвердить обработку |
| polymas.request_task | Предложить подзадачу; policy проверяет DAG, assignee и бюджет |
| polymas.publish_artifact | Зарегистрировать файл/объект с hash и ownership |
| polymas.request_operation | Запросить действие, требующее решения человека |
| polymas.get_operation | Узнать состояние без блокирования на минуты |
| polymas.report_progress | Публичная краткая сводка с доказательствами, не скрытое рассуждение |

Identity берётся из доверенного соединения + capability token, а не из senderId, переданного моделью. Разрешения проверяются на каждое обращение.

Outbox и состояние задачи пишутся одной транзакцией. Доставка at-least-once с messageId и inbox unique key (recipientId, messageId). Получатель получает упорядоченный inbox; общего глобального порядка всех агентов не обещается.

В exec новые сообщения передаются во вход следующего turn. Перед запуском сохраняется inbox batch с attemptId; только после наблюдаемого старта и подтверждения подачи входа отмечается delivered_to_context. Если возникла неоднозначность, batch сохраняет uncertain_delivery и повторяется с теми же ID для дедупликации в прикладной обработке.

Ограничения по умолчанию: message body ≤ 64 KiB, fan-out ≤ 32, hops ≤ 8, срок ≤ 24 h, максимум 1000 сообщений на run, очередь inbox ≤ 10000 элементов. Большие результаты — artifact refs. Значения изменяемы администратором в рамках workspace ceiling.

Ожидание ответа: агент отдаёт wait condition, turn освобождает слот; scheduler возобновляет при новом релевантном сообщении/deadline. На timeout — эскалация, fallback либо ошибка по task contract.

## 8. MCP gateway и границы полномочий

Gateway удерживает connection pools, tool allowlist, namespace bindings, audit и операции approval. По умолчанию один gateway-service на worker, а не отдельный тяжёлый DB server на каждого агента. Подключения разделяются по tenant/auth/scope и совместимости transport.

Если сервер stateful или требует выделенного stdio, manager создаёт выделенный child process для соответствующего security context; sharing никогда не смешивает credentials или namespace.

Gateway выдаёт агенту короткоживущий ограниченный токен по защищённому каналу/окружению. Upstream secrets остаются у manager; при необходимости прямого stdio server они передаются только этому дочернему процессу, не общему окружению Codex.

MCP tool annotations readOnly/destructive — подсказки, не доказательство безопасности. Read-only обеспечивается allowlist, DB permissions, filesystem sandbox и network policy.

Proxy должен сохранять negotiated capabilities, отмену, pagination, ошибки, ресурсы и prompts только в поддерживаемом объёме. Неподдерживаемые sampling/elicitation/roots объявляются недоступными, а не молча ломаются. Полная совместимость с произвольным MCP не обещается.

Для approval в exec gateway быстро возвращает operation_pending + ID. Модель завершает turn в waiting_user. После решения выполняется новая попытка с exact argument hash; долгое ожидание не подвешивает MCP до tool timeout.

Любой прямой доступ к внешнему write endpoint в обход gateway блокируется изоляционным профилем, если требуется строгая проверка операции. В trusted-local режиме границы честно описываются; он не допускается как межпользовательская песочница.

## 9. Данные и хранение

| Сущность | Ключевые поля |
|---|---|
| Workspace | id, schemaVersion, revision, defaults, retention |
| RoleVersion | roleId, version, instructionHash, runtimeDefaults, outputContract |
| AgentInstance | id, roleVersion, owningHolonId, configRevision, state |
| Holon | id, parentId, members, coordinatorAgentId, memoryBindingIds, contract |
| EdgePolicy | from, to, types, scope, enabled, revision |
| RuntimeSession | id, agentId, providerSessionId, authProfileId, hostId, compatibility |
| RuntimeProcess | id, agentId, attemptId, osPid, bootId, startTime, leaseEpoch |
| Task | id, runId, parentTaskId, dependencies, assignee, acceptance |
| Attempt | id, taskId, agentId, ordinal, inputHash, lease, outcome |
| Message | id, runId, sender, recipients, correlation, payloadRef, expiresAt |
| Inbox/Outbox | owner, messageId, cursor, delivery state, retryAt |
| Connection | id, recipeId/version, transport, desired, health, secretRefs |
| Binding | subjectId, connectionId, tools, namespaces, mode, revision |
| KnowledgeRecord | id, type, sourceRef/version, assertion, provenance, status |
| ArtifactVersion | id, hash, uri, mediaType, agentId, attemptId, acceptance |
| Approval | id, actionHash, scope, attemptId, expiresAt, status, actor |
| Event | id, streamId, seq, type, causation, time, payload, schemaVersion |
| AuthProfile | id, method, provider, hostId, secretHandle, capacityPolicy |
| Worker | id, bootId, capabilities, capacity, heartbeat, drain |

Секреты не содержатся в этих таблицах в открытом виде. Artifact body не помещается в event payload.

Структура локальных данных:

    .polymas/
      workspace.db
      config-snapshots/
      agents/<agent-id>/workspace/
      runtime-profiles/<profile-id>/
      artifacts/sha256/<prefix>/<hash>
      event-segments/
      backups/

Runtime-profiles ограничены OS ACL и не экспортируются. Working folders не включают чужие agent folders или host secrets.

SQLite WAL используется только на локальном диске; запись сериализуется и транзакции короткие. Журнал событий растёт в сегменты с retention; метаданные/индексы остаются в БД. Backup должен учитывать согласованный SQLite snapshot и referenced artifacts, а не простое копирование открытого файла.

В cluster PostgreSQL хранит задачи, lease, события и outbox; workers не используют общую сетевую SQLite. Outbox relay строит проекции и доставляет UI events. Отдельный broker вводится только при доказанном bottleneck и сохраняет DB как источник истины.

## 10. События и API приложения

Envelope: schemaVersion, eventId, streamId, sequence, timestamp, observedAt, workspaceId, runId, taskId, agentId, attemptId, correlationId, causationId, type, payload.

Порядок гарантируется по streamId/sequence. timestamp источника и observedAt управляющего контура различаются: часы hosts могут расходиться. При reconnect клиент сообщает cursor; backend отправляет snapshot + suffix событий. Если cursor удалён retention, возвращает resync_required.

Надёжные события: жизненный цикл, сообщения, approvals, артефакты, результаты инструментов. Эфемерные: визуальные particles, промежуточные text deltas, CPU samples. При backpressure допустима потеря/сжатие последних, но не первых.

Публичные команды v1:

| Команда | Результат и семантика |
|---|---|
| workspace.create/import/export | Workspace или экспорт без секретов |
| role.saveVersion | Неизменяемая версия |
| agent.createBatch | operationId + список IDs после commit |
| architecture.validate/commit | Диагностики + revision; optimistic concurrency |
| run.preflight/start | Report, затем runId; обязательный idempotency key |
| run.pause/cancel | Принятие команды; фактический итог через events |
| agent.sendMessage | durable receipt и получатель |
| connection.test/enable/revoke | Диагностика/новая revision |
| approval.decide | Accepted decision либо stale/rejected |
| artifacts.list/read | Метаданные и защищённый доступ |
| events.subscribe/replay | Cursor stream, фильтры с авторизацией |

Ошибки содержат code, safeMessage, retryable, fieldErrors, correlationId; stack trace и секреты не попадают в UI. Изменяющие команды требуют expectedRevision, если затрагивают конфигурацию.

Transport desktop — IPC; remote service отображает эти операции на versioned HTTP endpoints и authenticated event WebSocket. Это собственный канал PolyMAS, не экспериментальный Codex WebSocket.

## 11. Надёжность и внешние эффекты

Перед launch scheduler атомарно резервирует task + agent + slot и выдаёт lease с monotonically increasing epoch. Heartbeat каждые 5 s; lease TTL 30 s как стартовый параметр. Worker прекращает новые действия при истечении lease и завершает принадлежащие ему процессы.

Потеря lease не доказывает, что прежняя внешняя операция не произошла. Fencing проверяется gateway, artifact registry и локальными write locks; произвольный внешний сервис может fencing не поддерживать. Тогда итог unknown_external_effect и ручная сверка/compensation.

После падения daemon: восстановить durable state → обнаружить свои child processes по bootId/PID/start time → восстановить допустимые соединения или завершить orphan → reconcile attempts → только затем выдавать новые lease.

Прогресс stdout пишется в локальный ограниченный spool при разрыве с control service. Переполнение сначала ограничивает выдачу новых задач; при угрозе потери надёжных событий worker переходит drain и сохраняет причину.

Повторить можно безопасное чтение, незапущенный процесс или внешнюю операцию с подтверждённым idempotency key. При timeout внешней записи сначала status/reconciliation, не автоматическое повторение.

Windows supervisor применяет Job Objects для принадлежащих приложению деревьев процессов; Unix — process groups с grace и kill. OS resume/reboot тестируется отдельно. Нельзя завершать чужой процесс по одному совпавшему PID.

Диск full: запрет новых launch, попытка сбросить журнал, ограниченный аварийный audit, понятное состояние. Terminal success не подтверждается, если результат не сохранён.

## 12. Безопасность, данные и доступы

Threat model: вредоносный документ, MCP tool description/result, команда модели, скомпрометированный сервер, ошибочный оператор, другой tenant/worker.

Данные и сообщения рассматриваются как контент, не как распоряжения изменить политику. Составные промпты маркируют происхождение. Запрет расширения полномочий обеспечивается вне LLM.

Минимальные права filesystem/network, canonical paths, запрет traversal и выходов через symlink/junction, ограничение размера/времени, проверка URL/redirect destination, TLS для remote, secrets redaction до сохранения в логи.

Worker bootstrap использует одноразовое enrollment и короткоживущие identity credentials; tokens можно отозвать. UI users командного режима: viewer только читает, operator запускает/останавливает разрешённое, editor меняет конфигурацию, admin управляет доступом/секретами.

Markdown и tool output санитизируются; скрипты, локальные file URL и внешние загрузки не исполняются в renderer. Превью артефактов отделено от privileged window.

MCP authorization и учётные данные должны соответствовать выбранной версии протокола; bearer token не пересылается между сервисами с другим audience. [MCP authorization](https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization)

Retention по умолчанию: подробные run events 30 дней, диагностика 7 дней, конфигурация/принятые артефакты до удаления пользователем; срок изменяем. Экспорт данных и purge затрагивают индексы, кэш и backups по объявленному lifecycle. Удаление из локальной памяти не обещает удаления из провайдера.

## 13. Масштабирование до тысяч

`/api/state` разделяет последние задачи (`tasks`, ограниченная страница истории) и `agentActivity` (не более одной текущей записи на неархивный экземпляр). Активная/неизвестная попытка имеет приоритет над ожиданием владельца, очередью и ожиданием сообщения. Запись содержит только task ID, agent ID, state и lease. Граф и доступность остановки используют эту сводку; количество завершённых задач не может скрыть продолжающуюся попытку. Клиент продолжает проверять свежесть heartbeat. Оптимизация серверного индекса и полная пагинация истории проверяются отдельно.

Один управляющий проект может описывать N=5000 при малом C. Процессы запускаются лениво, каждый экземпляр сохраняет независимое состояние. Массовое создание не требует 5000 терминальных окон.

Для C=1000 scheduler распределяет 1000 выделенных Codex процессов по совместимым workers. Требования к ресурсам выводятся из benchmark RAM/CPU/process count и provider quota; количество hosts не фиксируется произвольно.

Не рассылать полный transcript каждому агенту. Контекст содержит роль, текущую задачу, bounded inbox и релевантные artifact refs. Сводки сохраняют provenance и явно маркируются как сжатые.

UI получает viewport/holon projections, delta batches не чаще 10–20 Hz, анимирует локально через requestAnimationFrame. История страницами; текстовый stream не вызывает rerender всех узлов.

Последовательность масштабирования: реальные 1 → 10 → 50 → 100 процессов, затем сертификационный C=1000 на разрешённом аккаунте/инфраструктуре. Симулятор на 5000 событийных агентов проверяет граф и очереди, но не считается проверкой 5000 CLI.

### Сохранённый результат внешней операции (FR-12)

Результат хранится атомарно с исходом операции и после записи неизменяем. Wire-представление — canonical JSON UTF-8, размер до 4 МиБ + 8192 байт (обёртка результата). Metadata содержит byteLength и SHA-256. Небольшой результат до 16 КиБ может быть inline; большой читается страницами 4–32768 байт с offset/nextOffset, без разрыва UTF-8 символа. Смещение вне документа/внутри символа отклоняется; последняя страница имеет nextOffset=null. Это только чтение ledger, без обращения к MCP и повторного эффекта. Owner API требует административный доступ; агентские страницы требуют исходный task/run/agent и действующий exact continuation с live∩pinned scope. Содержимое не попадает в activity events, диагностику и metadata списка.
