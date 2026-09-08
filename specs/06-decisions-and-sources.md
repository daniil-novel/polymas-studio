# 06. Архитектурные решения и источники

Проверено 08.09.2026. Эта дата относится к исследованию для спецификации; сведения внешних продуктов могут измениться.

## 1. Предлагаемые ADR

| ADR | Решение | Альтернативы / последствия |
|---|---|---|
| ADR-001 | Отдельный agentId, session и process на активного Codex-агента | Общий thread pool одного процесса нарушает исходное требование |
| ADR-002 | N и C — разные параметры | Нельзя обещать 1000 одновременно по числу карточек; start_all явно проверяет все slots |
| ADR-003 | Координатор и общая память независимы | «Ядро» не смешивает планировщик, LLM и БД |
| ADR-004 | Exec JSONL как начальный production-кандидат | Меньше интерактивных возможностей; app-server experimental отдельно |
| ADR-005 | Платформенный router/MCP для явных сообщений | Transcript сам по себе не даёт доказательства доставки другим агентам |
| ADR-006 | Event journal + projections, at-least-once с dedup | Exactly-once внешнего эффекта не обещается |
| ADR-007 | Tauri/React, Rust daemon, SQLite local | Один локальный модульный контур; PostgreSQL/remote worker позднее |
| ADR-008 | WebGL граф + текстовая альтернатива | UI-scale проверяется отдельно от runtime-scale |
| ADR-009 | Snapshot конфигурации на run | Правки роли/топологии безопасны и воспроизводимы; revocation отдельно |
| ADR-010 | MCP gateway с per-agent identity и ACL | Сложнее прямого подключения, но даёт аудит/отзыв/ограничения |
| ADR-011 | Холон — контракт группы, coordinator видимый | Нет скрытого дополнительного агента при каждой группе |
| ADR-012 | Секреты вне графа/экспорта/промптов | Auth compatibility и OS isolation требуют spike |

Все ADR имеют статус Proposed baseline. Они образуют конкретный проект для реализации; изменения фиксируются новой версией, а не неявно в коде.

## 2. Проверенные факты

| Факт | Источник | Практическое следствие |
|---|---|---|
| Локально установлен codex-cli 0.144.1 | codex --version, выполнено в рабочей среде | Это исходная обнаруженная версия, не обещание последней версии |
| exec имеет --json, явные sandbox modes, stdin prompt | Справка установленного CLI и [non-interactive docs](https://developers.openai.com/codex/noninteractive) | Возможен отдельный процесс и структурированный поток событий |
| exec resume принимает явный session ID | Справка exec resume и [официальное руководство](https://developers.openai.com/codex/noninteractive) | История связывается с экземпляром; --last не используется |
| ChatGPT login и API key — разные способы доступа | [Authentication](https://developers.openai.com/codex/auth) | Два пути в мастере, разные метрики/биллинг |
| app-server предоставляет thread/turn/approvals, статус experimental | [App Server](https://developers.openai.com/codex/app-server), локальная справка | Не выдавать экспериментальный адаптер за поддержанный production |
| Codex поддерживает STDIO/Streamable HTTP MCP | [MCP](https://developers.openai.com/codex/mcp) | Рецепты различаются по transport и auth |
| Filesystem MCP имеет allowed roots и write tools | [Официальный repository](https://github.com/modelcontextprotocol/servers/blob/main/src/filesystem/README.md) | Read-only требует дополнительных ограничений |
| Neo4j имеет self-hosted и Aura MCP пути | [Quickstart](https://neo4j.com/docs/mcp/current/quickstart/) | Разные мастера адреса/установки/авторизации |
| Neo4j config различает STDIO credentials и HTTP authentication | [Configuration](https://neo4j.com/docs/mcp/current/configuration/), [HTTP auth](https://neo4j.com/docs/mcp/current/authentication/) | Нельзя использовать одну форму credentials для всех transports |
| Tauri предоставляет capabilities для доступа frontend | [Capabilities](https://v2.tauri.app/security/capabilities/) | Privileged IPC ограничивается явно |
| Sigma использует WebGL для графов | [Sigma introduction](https://www.sigmajs.org/docs/) | Кандидат для больших графов, benchmark обязателен |
| SQLite WAL требует один host и не работает через network filesystem | [SQLite WAL](https://www.sqlite.org/wal.html) | Local SQLite, cluster PostgreSQL, не общий файл БД по сети |

Источники задают возможности внешних продуктов. Конкретные лимиты 5000 узлов, C, TTL, SLO, UX и стратегия очередей — проектные решения PolyMAS, а не утверждения этих источников.

## 3. Внешние ссылки для мастеров

- [Codex CLI](https://developers.openai.com/codex/cli) — установка и начало работы.
- [Codex authentication](https://developers.openai.com/codex/auth) — официальный вход.
- [OpenAI API keys](https://platform.openai.com/api-keys) — создание ключа пользователем.
- [Codex pricing / plan availability](https://developers.openai.com/codex/pricing) — проверять перед выпуском, не фиксировать числа в дизайне.
- [Codex MCP](https://developers.openai.com/codex/mcp) — базовая настройка клиента.
- [MCP authorization, versioned baseline](https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization) — принципы авторизации; конкретная protocol version согласуется при handshake.
- [Neo4j MCP quickstart](https://neo4j.com/docs/mcp/current/quickstart/) — self-hosted/Aura.
- [Neo4j configuration](https://neo4j.com/docs/mcp/current/configuration/) — переменные/параметры.
- [Filesystem MCP source](https://github.com/modelcontextprotocol/servers/blob/main/src/filesystem/README.md) — инструменты и roots.

## 4. Что требует решения/проверки до соответствующего выпуска

| Вопрос | Исходное предположение | Когда закрыть |
|---|---|---|
| Платформы | Windows 11 x64 первой | До installer M2 |
| Распространение | Локальное приложение; пользователь подключает собственный доступ | До M5 |
| Поддерживаемая CLI версия | Не автоматически latest; выбрать после smoke | G-01 |
| Общая авторизация многих процессов | AuthProfile на host, без копирования токенов | G-02 |
| Фактическая изоляция | Codex sandbox + OS граница по профилю | G-03 |
| Имя/бренд | PolyMAS Studio — рабочее название | До публичного релиза |
| Точный capacity C | По benchmark и provider access | G-07, M6 |
| Где находится Neo4j | Пользовательский существующий сервер или отдельная установка | M4 recipe |
| Командный backend и tenancy | Отдельные control service/users/workers | До M6 |
| Claude Code/API | Только extension contract сейчас | M7 |

Ни один пункт не мешает проектировать текущий интерфейс и вертикальный срез. Нельзя закрывать эти вопросы выдуманными лимитами или незаявленной автоматической авторизацией.

## 5. Ограничения проведённой проверки

Локально прочитаны справки CLI; auth.json, ключи и приватные диалоги не читались. Не выполнялись provider requests, установка MCP/БД или проверка 10/1000 реальных агентов.

Загрузчик объединённого Codex manual получил HTTP 403. Для фактов использованы точечные официальные страницы через OpenAI Docs MCP и официальные web-источники. Противоречие между онлайн-описаниями и установленным CLI решается проверкой выбранной версии; неподтверждённые флаги не переносятся в контракт.

Изображения сгенерированы встроенным image_gen. Это дизайн-референсы с демонстрационными именами/счётчиками; точность геометрического количества точек не является доказательством runtime.
