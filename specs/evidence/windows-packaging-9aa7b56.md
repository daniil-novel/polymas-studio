# Windows packaging — 9aa7b56

Дата: 2026-09-08. Срез D24a, **не production release**.

## Сборка

Исходники: `9aa7b565f37c6e04023ac122bd442000492dbd09`. На старте дерево Git чистое; runtime/frontend inputs во время сборки не менялись. Добавленный к концу packaging test не входит в binary. `npm run desktop:build`, `CARGO_BUILD_JOBS=1`, завершение 20:47:48 Europe/Moscow. Windows 10 Pro 10.0.19045 x64, около 32 GiB RAM; Node 25.2.1, npm 11.6.4, Rust 1.97.1 MSVC, Python 3.11.5. CI этого commit завершён success на Windows/Ubuntu и Chromium.

| Артефакт | Bytes | SHA-256 |
| --- | ---: | --- |
| `target/release/bundle/nsis/PolyMAS Studio_0.1.0_x64-setup.exe` | 8334723 | `3935D1AF3311D04596229CE67E50460377E77CBC4D7146C493EC3441DF2A6DA2` |
| `target/release/polymas-daemon.exe` | 18851840 | `FCAC6AFE80D8E54EC66F40FD495D451120949BDF853C1F4485923B550BE8EF76` |
| `target/release/polymas-desktop.exe` | 14843392 | `3E00154164EE049A4D8ED7BD045AB42AD01458A50B5F1F41CAE24DCDAEBDE5EA` |

NSIS currentUser, x64, выбор русского/английского языка. Authenticode: **NotSigned**. Установщик собран, но не установлен/обновлён/удалён в этой проверке. Native close/tray/drain, подпись и Windows 11 L1 остаются NOT VERIFIED. Артефакты локальные, в Git записаны исходники и доказательства; binary не опубликован как выпуск.

## Состав зависимостей

Отдельный snapshot получен через `git archive 9aa7b56`; пользовательские workspace/config/credentials не копировались. Rust SBOM: cargo-cyclonedx **0.5.9**, installed `--locked`, target `x86_64-pc-windows-msvc`, feature `polymas-desktop/custom-protocol`, `--describe binaries --format json --spec-version 1.5`. npm: официальный `@cyclonedx/cyclonedx-npm` **6.0.1**, из изолированной папки инструментов с `--ignore-scripts`; lockfile-only, JSON 1.5, прямой `--output-file`. Отдельно dependency tree с `--omit dev` и build tree без omit. Это состав разрешённых зависимостей, а не доказательство присутствия каждого байта package в tree-shaken JS.

Нормализатор сохраняет raw evidence. Он заменяет локальные Cargo identities на portable URN/purl и согласованно обновляет dependency edges. Неожиданные local paths, неверная identity/version, неизвестные ссылки, коллизии, подписанный input и некорректный serial отклоняются. При изменении документа version увеличивается, повтор идемпотентен. Он проверяет перечисленные инварианты, но не заменяет полный schema validator.

Все четыре нормализованных документа проверены `jsonschema 4.23.0` / Draft7 + FormatChecker по официальным `bom-1.5.schema.json`, `spdx.schema.json`, `jsf-0.82.schema.json`, полученным 08.09.2026 из [CycloneDX specification](https://github.com/CycloneDX/specification/tree/master/schema). Ошибок: **0**. Внешние schema refs разрешены из локальных файлов.

| Нормализованный документ | Верхнеуровневые components / dependency nodes | SHA-256 |
| --- | --- | --- |
| `tmp/daemon-sbom.json` | 202 / 203 | `69227c4a7c239dec2c1ec57e9ddd23abe93e63bc29b2638deb268d9f07eac416` |
| `tmp/desktop-sbom.json` | 350 / 351 | `7acb85f96088b129122c908ce3aa63a7dee135a01ae9b4f1c2782f190a99bd67` |
| `tmp/npm-runtime-portable.cdx.json` | 10 / 11 | `9fb06e9a7d1f2bd9ee04e830021f55489fe8eb3ad2d79f07260920077f993c7f` |
| `tmp/npm-build-portable.cdx.json` | 284 / 291 | `0c9a812049e685c178beb3aedf8ab175e0f128ab9f0fc2df4d7588e2472f9c74` |

Components могут быть вложенными. License inventory объединяет root и вложенные компоненты по name/version/purl. Runtime inventory из daemon + desktop + npm runtime содержит **370 записей**. Одна без license metadata: собственный `polymas-studio@0.1.0`. Политика исходников уже задана корневым LICENSE; generator не подставляет её автоматически. `reviewRequired` означает отсутствие metadata, а false не является заключением о выполнении лицензионных обязательств. Комплект third-party notices/текстов и проверка поставки ещё не завершены.

Первоначальный встроенный `npm sbom` записал `urn:uuid:***` и не прошёл schema validation. Эти raw файлы не квалифицированы и не включены в нормализованный комплект. RED regression закрепил отказ; защиту npm от раскрытия данных не отключали. Изолированный CycloneDX CLI с прямой записью дал корректные документы. npm metadata/install по закодированному package URL дважды получил ECONNRESET; pinned официальный tarball успешно установлен без scripts, проверки TLS сохранены.

## Повторить нормализацию

Требуется Python 3.11+, внешние генераторы устанавливаются отдельно. Пример для уже сгенерированных документов:

```powershell
python scripts/sbom.py normalize raw-daemon.cdx.json daemon.cdx.json --name polymas-daemon --version 0.1.0
python scripts/sbom.py normalize raw-ui.cdx.json ui.cdx.json --name polymas-studio --version 0.1.0
python scripts/sbom.py licenses licenses.json daemon.cdx.json desktop.cdx.json ui.cdx.json
python -m unittest discover -s tests/packaging -p 'test_*.py'
```

Перед поставкой обязательна полная schema validation; версии генераторов, входные lockfiles, source SHA, output hashes и target сохраняются вместе с комплектом. Новая генерация может иметь другое время/serial/hash; приведённые хэши идентифицируют только проверенные документы этой сборки. Автоматический release workflow — следующий срез D24.

## Regression и self-review

RED → GREEN: отсутствующий normalizer, npm basename вместо package identity, подписанный input, изменение без BOM revision increment, masked serial, перезапись input через CLI. Шесть tests покрывают сохранность raw/remote evidence и hashes, граф после переписывания local refs, идемпотентность, неверные версии/пустой граф/dangling refs/коллизии, path leaks и отсутствующие лицензии. Suite включён в Windows/Ubuntu CI и локальный spec check. Self-review: stdlib only, чтение до 16 MiB, без сети/секретов/выполнения зависимостей; metadata не выдумывается, output не может быть тем же файлом, что input.

Источники поведения генераторов: [cargo-cyclonedx](https://github.com/CycloneDX/cyclonedx-rust-cargo/tree/main/cargo-cyclonedx), [cyclonedx-npm](https://github.com/CycloneDX/cyclonedx-node-npm), [npm sbom](https://docs.npmjs.com/cli/v11/commands/npm-sbom/).
