# ADR-015: нативная граница редактора

Дата: 2026-09-08. Уточнение исполнения раздела 03, требования к отдельной службе и tray сохраняются.

`polymas-desktop-host` содержит проверяемый без WebView клиент и lifecycle нативной оболочки. Tauri вызывает ограниченные Rust-команды. WebView не получает control token, произвольный HTTP-клиент или shell. NativeConnection обращается только к закреплённому loopback порту, проверяет путь API, запрещает redirects, не использует proxy и ограничивает JSON-запрос 1 МиБ, ответ 16 МиБ. Ошибки сохраняют code/status; credential удаляется из диагностик.

Tauri shell и отдельный daemon упаковываются совместно. Привязка к рабочей области, reconnect и политика закрытия реализуются следующими срезами; наличие transport crate само по себе не означает готовое desktop-приложение.

Нативный startup использует `--adoption-timeout 30`. До подтверждения служба обслуживает control API, но не запускает задачи. Host сохраняет credential в OS vault и endpoint/reference в durable profile, затем отправляет authenticated adopt. Если host исчез до завершения протокола, служба закрывается по deadline, сохраняя старую очередь. Pending/adopted/expired меняются атомарно; поздний adopt не возрождает expired startup. Обычный явный daemon/dev запуск сохраняет прежнюю семантику.

Проверенные источники: [Rust commands](https://v2.tauri.app/develop/calling-rust/), [Tauri sidecars](https://v2.tauri.app/develop/sidecar/). Используется нативный вызов; JavaScript shell plugin не выдаётся окну.
