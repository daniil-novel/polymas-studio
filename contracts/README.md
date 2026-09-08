# Начальные контракты PolyMAS 0.1.0

Эти файлы — формальная часть спецификации для импорта архитектуры и ключевых событий. Они не являются API Codex и не запускают процессы.

| Файл | Назначение |
|---|---|
| [architecture.schema.json](architecture.schema.json) | Blueprint ролей, наборов экземпляров, холонов, связей, лимитов и MCP |
| [event.schema.json](event.schema.json) | Envelope нормализованного события и строгие payload трёх ключевых типов |
| [configuration-export.schema.json](configuration-export.schema.json) | Переносимая конфигурация конкретных экземпляров, версий ролей и placeholders подключений, по [ADR-018](../specs/adr/018-portable-configuration.md) |
| [single-agent.json](examples/single-agent.json) | N=1, C=1, прямой агент без ядра |
| [ten-agents.json](examples/ten-agents.json) | N=10, C=10, включая одного видимого координатора |
| [thousand-agents.json](examples/thousand-agents.json) | N=1000, C=1000, обязательный admission всех слотов |
| [events.jsonl](examples/events.jsonl) | Синтетические примеры событий; не журнал реальных агентов |

## Семантика blueprint

Population — инструкция массового создания, а не исполняющий агент и не общая история. При материализации каждого набора создаются count самостоятельных AgentInstance с постоянными UUID. Таблица соответствий populationId + ordinal → agentId сохраняется транзакционно.

Population count=1000 не допускает реализации 1000 виртуальных ролей в одном Codex. После admission запускаются отдельные процессы в соответствии с инвариантами спецификации.

coordinatorPopulationId ссылается только на набор из одного экземпляра. Его count включён в сумму N. Никакой дополнительный agentId не возникает из наличия холона.

Edge endpoint population:X задаёт разрешение для участников набора. Это не инструкция сделать broadcast. Dispatch определяет конкретных получателей отдельно; сообщение в runtime всегда имеет конкретные IDs.

auth:// и secret:// — внутренние ссылки на профили/секреты. Они требуют разрешения на целевой машине. modelSelection=runtime-default означает использование доступного default выбранного CLI без выдуманного имени модели.

## Валидация сверх JSON Schema

JSON Schema проверяет структуру. Domain validator обязан дополнительно проверять:

1. Уникальность IDs внутри каждого набора сущностей.
2. Существование roleId/roleVersion и connectionId у ссылок.
3. Общую сумму экземпляров 1–5000 и C ≤ N.
4. Ровно один экземпляр у координатора.
5. Отсутствие циклов владения холонов и единственного владельца population.
6. Корректность edge endpoints и ACL, отсутствие неподдерживаемого self-send.
7. При preset=single: N=1, C=1, без координатора; при star координатор обязателен.
8. При sharedMemoryConnectionId: существующее включённое подключение с подходящим binding.
9. Наличие доступного auth profile/runtime/required MCP при preflight.
10. Совместимость путей, sandbox, protocol versions и capabilities.

Runtime validation не заменяется проверкой JSON. Наличие корректного файла не подтверждает квоту или доступность инфраструктуры.

Экспорт материализованной конфигурации имеет отдельный format/schemaVersion/migrationVersion и не является blueprint populations. Два [примера](export-examples/materialized.json) проверяются JSON Schema; Rust regressions сравнивают с ними полный результат реальной сериализации (только случайные ID нормализуются). Импорт требует новых identities, проверки ссылок/версий/топологии и повторной настройки подключений. Экспорт не содержит runtime/connection values, но сохраняет свободные тексты владельца; перед передачей файла их следует просмотреть.

## Версионирование событий

sequence монотонен внутри streamId. Event ID уникален. causationId может ссылаться на другое событие; correlationId связывает сценарий целиком.

Строгие payload описаны для agent.state.changed, message.delivered, mcp.call.completed. Другие виды перечислены функционально в архитектуре и добавляются к схеме по мере принятия соответствующих срезов. Envelope сохраняет неизвестные типы, но consumer не должен трактовать их как успех или расширение полномочий.

Перед production каждого среза в контракт добавляются его конкретные request/response/payload schemas и negative fixtures. Это исходный набор контрактов, не утверждение о готовом полном backend API.

## Проверка текущего пакета

scripts/validate_spec.py проверяет schema validity, примеры, смысловые инварианты blueprint и локальные ссылки документов. Нужен Python 3.11+ с jsonschema. Эта проверка не запускает Codex, MCP или сетевые обращения.
