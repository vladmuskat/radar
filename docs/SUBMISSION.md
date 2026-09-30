# Комплект для проверки тестового задания

Проект представляет собой локальный прототип тренажёра оператора РЛС на Rust, TypeScript и Tauri 2. Он работает без внешнего сервера и интернета: симуляция, проверка действий оператора, авторизация и сохранение данных выполняются в Rust, интерфейс построен на React, автономная карта поставляется вместе с приложением.

## Быстрый запуск

Требуются Node.js 22+, Rust stable MSVC, WebView2 Runtime, Visual Studio C++ Build Tools и Windows SDK.

```powershell
npm ci
npm run desktop
```

Учётные записи для демонстрации:

| Логин      | Пароль     | Роль          |
| ---------- | ---------- | ------------- |
| `operator` | `operator` | Оператор      |
| `admin`    | `admin`    | Администратор |

Production-сборка:

```powershell
npm run desktop:build
```

Исполняемый файл появится в `target/release/radar-trainer.exe`.

## Где начинать чтение кода

1. `crates/radar-core/src/engine/mod.rs` — жизненный цикл и шаг симуляции.
2. `crates/radar-core/src/engine/detection.rs` — проход луча, обнаружение и уничтожение.
3. `src-tauri/src/main.rs` — application service, IPC и отдельный поток симуляции.
4. `src-tauri/src/ports.rs` — абстракции авторизации и хранилища.
5. `src/App.tsx` — пользовательские сценарии и синхронизация UI.
6. `src/RadarMap.tsx` — визуализация MapLibre и взаимодействие с отметками.

Назначение всех именованных функций и методов перечислено в [CODE_REFERENCE.md](CODE_REFERENCE.md). Развёрнутое описание границ компонентов, потока данных и паузы находится в [ARCHITECTURE.md](ARCHITECTURE.md).

## Структура

```text
crates/radar-core/   Независимое Rust-ядро симуляции без Tauri, UI и базы данных
src-tauri/           Desktop-оболочка, IPC, авторизация, SQLite и логирование
src/                 React/TypeScript-интерфейс и карта MapLibre
public/maps/spb/     Готовые автономные векторные тайлы Санкт-Петербурга
tests/               Frontend unit-тесты и автономный preview карты
scripts/             Windows-запуск и воспроизводимая подготовка карты
docs/                Архитектура, справочник кода, тестирование и масштабирование
```

## Ключевые решения

- Rust является единственным источником времени, положения целей, пересечения луча и оценки ответа.
- React получает DTO без истинного типа цели и не может самостоятельно определить БВС.
- Пауза останавливает simulation clock; WebSocket не нужен, поскольку Tauri Channel передаёт снимки из локального worker-потока.
- Двойной щелчок сразу фиксирует решение, но удаление цели выполняется ядром при следующем проходе луча.
- SQLite скрыта за `Repository`, а авторизация — за `IdentityProvider`, поэтому PostgreSQL или серверный провайдер можно подключить без изменения симуляции и UI-контракта.
- Карта и её лицензия включены в поставку; сетевые API и ключи не требуются.
- Файловые JSONL-логи сохраняются в `%LOCALAPPDATA%\ru.radar.trainer\logs`.

## Проверка

```powershell
npm run test:frontend
npm run test:rust
npm run check
npm run format:check
npm run format:rust
npm run map:check
```

## Подготовка чистой папки

Из корня рабочего репозитория:

```powershell
npm run submission:create
```

Команда пересоздаёт `submission/RADAR` из текущего working tree. В комплект не попадают `.git`, зависимости, сборки, локальные БД, исходный PBF и внутренние референсные скриншоты. Папку `submission/RADAR` можно архивировать и передавать на проверку.
