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
- [ ] D15. Формы ролей и массового создания экземпляров.
  - [x] D15b. Форма архитектуры и холонов с сохранением через ревизии; ресурсные scaling rules далее в D20.
  - [x] D15a. Роль, новая версия, массовое создание, список экземпляров и stale-form guard; расширенные действия экземпляров далее.
- [ ] D16. WebGL-граф, поиск, layout worker и доступный список.
  - [x] D16a. Проекция состояний/связей и bounded deterministic layout; renderer следует отдельно.
  - [x] D16b. Sigma renderer, worker, миникарта, фильтры и виртуальный список; drag/pinning, агрегация холонов и расширенная клавиатурная навигация далее.
- [ ] D17. Задачи, чат, результаты, управление выполнением.
  - [x] D17a. Composer и прямой запрос агенту, адресация/критерии/stop policy; результаты и приёмка в UI далее.
  - [x] D17b. Список последних запусков, результат, приёмка и действия; полная пагинация истории/вложения/редактор DAG далее.
- [ ] D18. MCP-мастер, bindings, диагностика и approvals в UI.
  - [x] D18a. HTTP lifecycle подключений и protected secret commands; UI/gateway далее.
  - [x] D18b. Каталог/мастер и bindings в UI; atomic activation, OAuth, удаление/ротация и approvals UI далее.
- [ ] D19. Знания, журнал, replay и история конфигурации в UI.
- [ ] D20. Масштабирование по правилам и ресурсные ограничения.
- [ ] D21. Удалённые workers, enrollment, fencing и кластерный storage.
- [ ] D22. Реальные Codex smoke/resume/concurrency и failure tests.
- [ ] D23. Browser e2e, computer use и визуальная сверка трёх экранов.
- [ ] D24. Упаковка Windows, SBOM, release workflow и инструкции.
- [ ] D25. Финальный self-review, README, changelog и матрица приёмки.

## Внешние условия для выпуска

- GitHub: приватный `daniil-novel/polymas-studio` создан, privacy проверена, история отправлена через сохранённый вход Git.
- Подписанный installer: нужен сертификат владельца/канал подписи; ключи не включаются в репозиторий.
- Реальный C=1000 и 8-часовой soak требуют доступного стенда, квоты и измерений. Simulator не подтверждает AC-24.
- Реальный Neo4j требует доступного endpoint и credentials; connector protocol tests не заменяют эту проверку.
- Claude/API остаются будущими адаптерами по FR-17; первый исполняющий backend — Codex.

## Правила доказательств

Не снимать требования ради зелёного отчёта. Обозначать отдельно IMPLEMENTED, VERIFIED, BLOCKED и NOT IMPLEMENTED. Фиксировать конкретный тест, результат и ограничения в `specs/evidence/`. Не объявлять выпуск production-ready, пока обязательные gate не пройдены.
