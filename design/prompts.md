# Промпты генерации дизайна

Встроенный image_gen; CLI/API fallback не использовался. Отдельного system-поля инструмент не имеет: системный бриф включён в роль и требования передаваемого prompt. Ниже сохранены бриф и точные тексты трёх запросов.

## Системный бриф дизайнера

Ты — ведущий продуктовый дизайнер и архитектор интерфейсов production-приложения PolyMAS Studio. Создай реализуемую настольную среду проектирования и наблюдения холонических ИИ-систем. Пользователь создаёт ролевые инструкции и независимые экземпляры; каждый работающий Codex-агент имеет свой процесс и историю. Отделяй число экземпляров от параллелизма, координацию от памяти, агента от источника знаний. Интерфейс должен давать понятные операции, реальные статусы и путь исправления ошибок. Вдохновляйся пространственной рабочей областью Obsidian, но создай самостоятельную визуальную систему. Тёмный графит, спокойные панели, мятные агенты и фиолетовая память; эффекты только для наблюдаемых действий. Не показывай фиктивные мысли, интеллект в процентах или несуществующие проверки. MCP-переключатель должен вести в понятный мастер с источником каждого параметра и проверкой. Проектируй большие системы через холоны, агрегацию и доступный список. Используй русский интерфейс, реалистичные поля, пустые/ошибочные состояния и клавиатурный доступ. Итог — полноэкранные плоские UI-референсы, а не рекламные изображения. Числа на референсе демонстрационные; нормативная семантика описана в спецификации.

## Запрос 01

Use case: ui-mockup
Asset type: high-fidelity desktop application design reference, screen 01 of PolyMAS Studio.
You are a senior product designer for a real professional desktop application for authoring and operating holonic AI agent systems. Translate requirements into a precise, implementable interface. Show one complete flat app screenshot, not a device mockup or marketing page. Russian UI. Landscape 16:10, high resolution, crisp typography. Obsidian-inspired knowledge graph workspace, original visual identity. Graphite near-black #101319 background, subtly lighter panels #181D25, hairline borders #2B3442, off-white text #E7EDF5, electric mint #6DE0B3 active agents, muted violet #A699F0 central memory. Restrained bloom only on active nodes and edges, no glossy 3D, no brain illustration, no decorative gradients.
Screen composition: compact top bar at 54px, left explorer 220px, huge center interactive graph workspace, right inspector 300px, bottom event timeline 160px. Small PolyMAS wordmark top left. Top breadcrumb 'Рабочая область / Исследование рынка', chip 'Codex CLI', status '24 агента · 6 работают', controls 'Пауза' and 'Остановить'. Thin amber strip 'ДЕМО · концепт интерфейса'.
Left navigation: 'Обзор', selected 'Граф', 'Агенты', 'Задачи', 'Знания', 'MCP', 'Запуски'. Under 'ХОЛОНЫ' show nested groups 'Исследование 12', 'Проверка 8', 'Синтез 4'; footer '+ Создать агента'.
Center graph: 24 individual agent dots grouped into three subtle open rounded cluster regions with tiny labels. Each dot means an individual instance, not a model. Several labels: 'Аналитик · 01', 'Критик · 02', 'Редактор · 01'. One selected mint node 'Аналитик · 03'. Dotted soft edges for configured connections, only three bright thin directional mint edges with traveling dots for observed messages. A separate violet small hexagonal node 'Общая память', with one violet active access edge. Include lots of negative space and dotted grid barely visible. Top graph segmented controls 'Агенты' / 'Знания' / 'Потоки', active first. Bottom-right minimap and zoom 100%, bottom-left compact legend 'Работает', 'Сообщение', 'Обращение к памяти'. No fake thought bubbles.
Right inspector: title 'Аналитик · 03', status 'Выполняет задачу', small monospaced id 'agent-003', role 'Исследователь', runtime 'Codex CLI', identity field 'Отдельный процесс', task 'Сравнить источники'. Tabs 'Обзор' 'События' 'Чат'. Section 'Последние действия': 'Прочитал 3 документа', 'Отправил сообщение Критику · 02', 'Запросил общую память'. Small permissions row 'Файлы: чтение' and MCP chips 'Файлы' 'Neo4j'. Button 'Открыть диалог'.
Bottom event panel full center width labeled 'Ход выполнения' with time ticks and three clean log rows for task start, actual agent message, memory read. Include a compact task composer: 'Опишите задачу для системы…' target chip 'Вся система', send arrow.
Everything should look usable, precise and buildable with real controls; no invented percentage intelligence or ROI scores, no terminals full of code, no excessive tiny text. Beautiful professional systems tool, polished but quiet.

## Запрос 02

Use case: ui-mockup
Asset type: PolyMAS Studio desktop screen 02, role and instance editor, professional high fidelity flat screenshot.
You are a senior product designer converting the user's holonic multi-agent editor requirements into a buildable product. Russian interface. Same design system as the supplied reference: dark graphite #101319, panels #181D25, fine #2B3442 borders, mint #6DE0B3 selected controls, off-white crisp text, restrained violet memory accents. Match its app shell, sidebar, font scale and compact information density. Landscape 16:10. The reference is a STYLE reference only. Create a new screen, not an overlay on the same graph.
Top bar: PolyMAS / Рабочая область / Агенты. Amber narrow 'ДЕМО · концепт интерфейса'. Sidebar labels Обзор, Граф, selected Агенты, Задачи, Знания, MCP, Запуски.
Main title 'Новая роль и экземпляры', subtitle 'Одна роль. Независимые агенты.' Horizontal tabs 'Роль', 'Экземпляры', 'Связи', 'Доступ'. Large two-column form. Left wide column section 'Ролевая инструкция' with field 'Название роли' value 'Исследователь', then a large editor containing 5 comfortably readable Russian lines:
'Ты — исследователь источников.'
'Проверяй факты и сохраняй ссылки.'
'Отделяй выводы от предположений.'
'Передавай спорные утверждения критику.'
'Результат: записка с источниками.'
Below compact fields 'Среда выполнения: Codex CLI', 'Вход: ChatGPT', 'Модель: Из доступных в аккаунте'. Section 'Доступ к знаниям' with chips 'Файлы · только чтение', 'Общая память · чтение'.
Right column section 'Экземпляры' with number stepper large value '12' and a short slider marked 1, 10, 100, 1000. Subtle helper 'У каждого свой ID, история и процесс при запуске'. Field 'Шаблон имени' value 'Исследователь · {n:02}'. Section 'Одновременно работают' stepper '4', note 'Остальные ожидают в очереди'. Dropdown 'Правило масштабирования: Фиксированное число'. Compact preview list 4 rows: 'Исследователь · 01' / 'agent-001' / 'Новый', similarly 02,03,04, and '+ ещё 8'. A tiny tree diagram for holon Исследование with four individual agent dots, no active glows.
Footer fixed action bar 'Будет создано: 12 агентов · роль v1' with secondary 'Отмена' and mint primary 'Создать 12 экземпляров'. Elegant professional product UI, no fake live data, no ROI, no unnecessary charts, no brain illustration. All switches have labels, text readable, spaced aligned grid, polished editor not marketing.

## Запрос 03

Use case: ui-mockup
Asset type: PolyMAS Studio desktop screen 03, MCP connections and guided setup, high-fidelity implementable application.
You are a senior product designer for a real holonic AI agent editor. Create one complete flat desktop screenshot. Russian UI, landscape 16:10. Use the reference image only for its consistent original dark graphite #101319 design system, mint #6DE0B3 interactions, violet #A699F0 memory accents, white crisp text, compact borders, sidebar and header. No device frame, no marketing decorations.
Top bar PolyMAS / Рабочая область / MCP, thin amber strip 'ДЕМО · концепт интерфейса'. Sidebar Обзор, Граф, Агенты, Задачи, Знания, selected MCP, Запуски.
Content left 58 percent, right setup drawer 42 percent. Title 'Подключения MCP' subtitle 'Инструменты и знания для ваших агентов'. A clean vertical list of four connector rows, not giant marketing cards: 1 folder icon 'Файлы', status green check 'Подключено', subtitle 'Чтение · 12 агентов', toggle on; 2 violet database icon 'Neo4j', amber status 'Требуется настройка', subtitle 'Граф знаний', toggle grey OFF, selected border; 3 database 'PostgreSQL', grey status 'Отключено', toggle off; 4 link 'Свой MCP-сервер', button 'Добавить'. Under list an explanatory small section 'Область доступа' with chips 'Вся система', 'Холон', 'Агент'; note 'Подключение не предоставляет доступ автоматически'.
Right drawer title 'Подключить Neo4j', subtitle 'Графовая память', close button. Horizontal stepper '1 Сервер' '2 Доступ' '3 Проверка', active 2. Clear guidance card 'Как получить доступ': numbered three brief lines '1. Откройте консоль вашей базы Neo4j.' '2. Найдите адрес подключения и имя базы.' '3. Создайте учётную запись только для чтения.' External link 'Открыть инструкцию ↗'. Segmented transport 'Локальный сервер' active / 'HTTPS'. Fields 'Адрес базы' placeholder 'neo4j+s://…', 'База' value 'neo4j', 'Имя пользователя' placeholder 'polymas_reader', 'Пароль' masked with eye icon and small lock 'Сохраняется в защищённом хранилище'. Switch 'Только чтение' on. Small helper 'Сервер MCP и база данных проверяются отдельно'. Footer outlined button 'Проверить подключение', disabled primary 'Подключить'. Small status 'Проверка ещё не выполнена'. No green successful check for untested connection. This is a settings interface with concrete readable steps, deliberate spacing, accessible labeled toggles, no code wall or terminal.

## Входные референсы и результаты

01 создан с нуля. В запросах 02 и 03 изображение design/references/01-agent-graph.png передано как стилевой референс.

- 01 → references/01-agent-graph.png
- 02 → references/02-role-instances.png
- 03 → references/03-mcp-setup.png

Результаты просмотрены. Условные счётчики, все зелёные узлы и прочие отклонения генерации нормализованы в design-spec.md; изображения не считаются точным тестом поведения.

