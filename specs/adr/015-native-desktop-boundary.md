# ADR-015: нативная граница редактора

Дата: 2026-09-08. Уточнение исполнения раздела 03, требования к отдельной службе и tray сохраняются.

`polymas-desktop-host` содержит проверяемый без WebView клиент и lifecycle нативной оболочки. Tauri вызывает ограниченные Rust-команды. WebView не получает control token, произвольный HTTP-клиент или shell. NativeConnection обращается только к закреплённому loopback порту, проверяет путь API, запрещает redirects, не использует proxy и ограничивает JSON-запрос 1 МиБ, ответ 16 МиБ. Ошибки сохраняют code/status; credential удаляется из диагностик.

Tauri 2.11.5 и отдельный daemon собираются совместно. Профиль хранится в OS local app data `studio.polymas.desktop`, workspace по умолчанию — `workspaces/default`. Системный vault namespace — `PolyMASStudio/desktop`; WebView не получает его записи. Окно использует те же React assets, что проверяемый browser-клиент. Windows GUI и Linux worker — отдельные targets; библиотека domain и daemon проверяются на обеих ОС.

Окно `main` имеет только шесть app commands и listen/unlisten. Неизвестное окно и внешний origin отклоняются capabilities; navigation закреплён на app origin (в development также точный localhost:1420), новые WebView запрещены. CSP не разрешает произвольные сетевые запросы или frames. Внешние инструкции требуют отдельного ограниченного opener (ещё не подключён).

Single-instance возвращает к существующему окну. Первое закрытие требует выбор; background скрывает окно, tray позволяет вернуться, сбросить выбор или явно остановить. Stop ждёт освобождения workspace lock. Если host не получил подключение, занятый workspace запрещает сообщить успешную остановку. Startup/stop сериализованы. Текущий launcher capacity=1; UI capacity, выбор workspace, installer drain и подпись — незаконченные срезы, а не снятые требования.

Для binary и lib tests явно включён Common Controls v6 через linker manifest; ресурс Tauri содержит icon без второго manifest. Это устраняет STATUS_ENTRYPOINT_NOT_FOUND тестового exe и duplicate MANIFEST при сборке binary, не меняя системные библиотеки. [Microsoft manifest dependency](https://learn.microsoft.com/en-us/cpp/build/reference/manifestdependency-specify-manifest-dependencies?view=msvc-170).

Нативный startup использует `--adoption-timeout 30`. До подтверждения служба обслуживает control API, но не запускает задачи. Host сохраняет credential в OS vault и endpoint/reference в durable profile, затем отправляет authenticated adopt. Если host исчез до завершения протокола, служба закрывается по deadline, сохраняя старую очередь. Pending/adopted/expired меняются атомарно; поздний adopt не возрождает expired startup. Обычный явный daemon/dev запуск сохраняет прежнюю семантику.

Проверенные источники: [Rust commands](https://v2.tauri.app/develop/calling-rust/), [Tauri sidecars](https://v2.tauri.app/develop/sidecar/). Используется нативный вызов; JavaScript shell plugin не выдаётся окну.

[Single instance](https://v2.tauri.app/plugin/single-instance/), [System tray](https://v2.tauri.app/learn/system-tray/) сверены 2026-09-08. Нативная visual QA отдельно от MockRuntime IPC regression и browser screenshot.
