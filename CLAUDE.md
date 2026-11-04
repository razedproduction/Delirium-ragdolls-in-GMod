# CLAUDE.md

DMR — аддон Garry's Mod: Delirium (active ragdoll в стиле NaturalMotion) в режиме **FOLLOW** на **собственных
рэгдоллах GMod**, NPC и моделях из Workshop. Собственное тело NM (build Homer из GTA IV игрока) подгоняется под скелет
модели (`tools/dms-fit`), форк VPhysics-Box3D делает его нативно под моделью, Delirium каждый физический шаг
возвращает силы и моменты, модель позируется по частям. План — [ROADMAP.md](ROADMAP.md), решения —
[docs/adr/](docs/adr/README.md) (раскол с golden-проектом — [ADR 0036](docs/adr/0036-split-from-golden.md)).

С пользователем общаться **по-русски**.

## Жёсткие правила

- **Golden-проект — соседний чекаут `E:\GitStuff\MyRepos\DMS in Gmod`** (`razedproduction/Delirium-physics-in-GMod`,
  только Homer, один в один с Playroom). Отсюда он — **только чтение**: ничего там не менять, не собирать в его
  `target/`, не переключать ветки, не коммитить. Ядро берётся оттуда path-зависимостями, никогда не копией:
  `tools/dms-game` (данные NM из игры), `tools/dms-vphysics` (`IPhysicsNM`, привязка, нативные тела, хуллы, хост и
  куски FOLLOW без Lua), `tools/dms-ctl` (канал стенда, RCON, сценарии, записи) и форк `vphysics/` (собирается там
  его `scripts\build-vphysics.ps1`). Неточности ядра и физики чинятся **там**, со сверкой с Playroom, отдельной
  задачей в том репозитории (сначала показать расхождение), и приходят сюда оттуда. Сборка требует состояния golden
  не старше его PR #39 (`dms_vphysics::hull`).
- **Homer здесь нет**: ни тела, ни модели, ни спавна, ни golden-сценариев Homer. Шаблон подгонки — по-прежнему build
  Homer из данных NM (`dms_game::Game::homer`), это не «тело Homer». `models/dms/homer.mdl` — дело аддона golden.
- **Delirium не тюним**: те же входные данные NM, drives как есть, без масштабирования и фильтров; единственное
  исключение — компенсация гравитации (ADR 0007). Слой реакций — код Playroom через крейт `delirium` (ADR 0016), не
  своя таблица. Пользовательский тюнинг из меню (ADR 0009) — только через публичный интерфейс NM, по умолчанию выключен.
  Эталона Playroom для подогнанного тела нет (Playroom не грузит подогнанный build): эталон — тот же build в LEAD на
  крейте (`dmr-check fit`), тело сверяется с build (`dmr-check body`).
- `E:\GitStuff\MyRepos\delirium-physics (CLAUDE DONT CHANGE ANYTHING HERE)` — **только чтение**, ничего там не
  менять, не собирать в его `target/`, не запускать `cargo fix`. Подключается path-зависимостью (ADR 0001).
- `E:\GitStuff\MyRepos\delirium-physics (gmod addon & api)` — прошлая попытка, только справочник, код не переносить;
  её `delirium-ragdoll` (`--fit-model`, `--reskin`, `--nm-body`, `.nmfit.txt`) игнорировать полностью.
- **Свои имена, чтобы ставиться рядом с golden**: модуль `gmsv_dmr_win64.dll` (`require("dmr")`, глобал `DMR`),
  аддон `garrysmod/addons/dmr` (`lua/dmr/`, `autorun/*/dmr_*.lua`), API `dmr.*`, convar'ы и команды `dmr_*`, хуки и
  net `DMR_*`/`dmr_*`, поля сущностей `DMR_*`, `data/dmr/`, порт канала стенда 27098. Ничего с именами `dms`/`DMS`
  в Lua и модуле не заводить (исключение — ключ `dms_skeleton` в файлах ригов, формат игроков). Свой предмет
  Workshop (не 3829811829).
- Только **win64** (`x86_64-pc-windows-msvc`). Никаких win32/x86/thiscall.
- **Без AVX2**: у пользователя процессор без AVX2 — никаких `target-cpu=x86-64-v3`, `+avx2`, `+fma`.
- Игровые данные GTA IV и всё, что из них сделано (build, записи `fit/out/`), в репозиторий и релизы не попадают
  (ADR 0006). Лицензия GPL-3.0-only.
- **Работа через PR.** В `main` напрямую не коммитить и не пушить. Каждая веха или правка — своя ветка от свежего
  `origin/main` (`git fetch` + `git switch -c <ветка> origin/main`), затем `git push -u origin <ветка>` и
  `gh pr create --base main` (repo `razedproduction/Delirium-ragdolls-in-GMod`). Мерж PR делает только пользователь.
  Каждая веха ROADMAP — в отдельной новой сессии. Сессии в одной папке не переключают чужую ветку.

## Сборка, установка, проверка

```powershell
.\scripts\build.ps1            # -> dist\gmsv_dmr_win64.dll (release; -DebugBuild для debug)
.\scripts\install.ps1          # dll -> garrysmod\lua\bin, addon\ -> garrysmod\addons\dmr (-Fork: форк из golden-чекаута)
cargo build --release -p dmr-ctl -p dmr-check -p dms-rig
```

- GMod: `E:\Games\Steam\steamapps\common\GarrysMod` (x86-64 ветка, форк `vphysics.dll` уже в `bin\win64`, оригинал —
  `vphysics.dll.original`; там же может стоять golden — `gmsv_dms_win64.dll`, `addons\dms`). После сборки
  устанавливать в игру самому, не спрашивая.
- В PowerShell не запускать `build.ps1` с `2>&1`: вывод cargo в stderr превращается в ошибку.
- Стенд ([docs/bench.md](docs/bench.md)): **сама игра в окне** — `.\scripts\server.ps1 start|stop|restart|status`
  (listen-сервер на `127.0.0.1`, окно 1280×720; `-Hidden` — `-textmode`) и `.\target\release\dmr-ctl.exe`: `status`,
  `exec`, `lua <код>` / `lua -f <файл>`, `nm`, `bench reset|stop|cancel`, `snapshot`, `watch --for 2s` (**всегда с
  `--for`**), `run fit\scenarios\<имя>.txt --fit <model.mdl> [--lead]`. Сверки — `.\target\release\dmr-check.exe`:
  `body [<mdl>]` (подогнанное тело против build и SDK `nm_build_fit`), `fit [--model <mdl>] [имена]` (FOLLOW против
  LEAD того же build); папка GTA IV — `--game` (запоминается в `fit\game_dir.txt`) или `DMR_GAME_DIR`; `server.ps1`
  берёт её оттуда же или из `golden\game_dir.txt` golden-чекаута. Поведение проверять там самому, пользователю
  оставлять визуальное. Пока стенд запущен, пользователь не может играть — после проверки `server.ps1 stop`. Стенд
  golden и этот — одна и та же игра: не запускать оба. В PowerShell 5.1 `"` в аргументах `dmr-ctl` теряются: в Lua —
  одинарные кавычки или `lua -f`. Канал — только `127.0.0.1`, только при `dmr_dev 1` (через одноразовый
  `data/dmr/bench.txt`). Выделенный сервер с форком не работает и не используется.
- Оба модуля могут быть загружены вместе, но форк держит один step-callback на окружение: NM на теле у одного модуля
  останавливает агентов другого (ROADMAP «Next»).
- Релиз: `.\scripts\package.ps1` → `dist\DMR-<версия>-win64.zip` (`gmsv_dmr_win64.dll` + форк из golden-чекаута, без
  Lua: аддон — в своём предмете Workshop; установщик — `scripts\release\install.bat`). Версия — `Cargo.toml`
  (`workspace.package`) и `DMR_ADDON_VERSION` в `dmr_init.lua`, одинаковые. Публикация — только с разрешения
  пользователя.

## Устройство

- `module/` — `gmsv_dmr` (Rust, `cdylib`): копия модуля golden на момент раскола без Homer (`ragdolls.rs` — подгонка,
  нативные тела, позы модели). Общий FOLLOW-код меняется в golden и переносится сюда руками, пока ядро там не станет
  библиотекой (его ADR 0037). `lua.rs` — LuaJIT API через `GetProcAddress`, нативы через `lua::native!`.
- `addon/lua/` — серверный и клиентский Lua: `autorun/server/dmr_init.lua` (`require("dmr")`), `dmr/*.lua`
  (`ragdolls.lua`, `deaths.lua`, `live.lua`, `skeletons.lua`, `rig_view.lua`, реакции, окно `cl_menu.lua`, инструмент).
- `tools/dms-fit` — подгонка build под скелет модели (общая с модулем и редактором); `tools/dms-rig` — редактор ригов
  (egui, [docs/rig-editor.md](docs/rig-editor.md)); `tools/dmr-ctl` — CLI стенда поверх `dms-ctl` из golden;
  `tools/dmr-check` — сверки подогнанного тела; `fit/scenarios` — сценарии Playroom для `dmr-check fit`.

Инварианты:
- Внутри step-callback (`SetStepCallback`, внутри `Simulate`) — **никакого Lua и игровых вызовов**, только методы
  `IPhysicsNM`; обмен с Lua через очереди команд/событий (ADR 0003).
- Без `IPhysicsNM` (`VBOX3D_NM_006`) модуль загружается, но ничего не делает и объясняет почему (ADR 0005).
- Единицы на границе с NM — SI, Z вверх, 1 unit Source = 0.0254 м.
- **`sv_gravity` не меняем никогда**. NM считает с 9.8 м/с², Source — 15.24 м/с²; разницу снимает сила
  `m_k · (g_nm − g_src)` на каждую часть (ADR 0007). Drives NM не масштабировать.

## Стиль

- Код как в `module/src/*.rs`: краткие `///`-комментарии, `// SAFETY:` у каждого `unsafe`, без лишних зависимостей.
- Документация (README, ROADMAP, `docs/` с ADR, `fit/README.md`, заметки к релизу) — **на английском**; с
  пользователем по-русски.
- Новое архитектурное решение — новый ADR `docs/adr/NNNN-slug.md` + строка в индексе; принятые ADR не
  переписывать, а заменять.
- Выполненные пункты вехи отмечать `[x]` в ROADMAP.md (в ветке вехи, вместе с её PR).
