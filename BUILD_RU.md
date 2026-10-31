# Сборка MouseTweaks-RS (.hmod) из исходников

## 1. Требования

- Rust (стабильный, проверено на 1.99) + rustup: https://rustup.rs
- Python 3 (используется сборочным скриптом для упаковки ZIP и правки JSON)
- Кросс-линкер под Windows: `x86_64-w64-mingw32-gcc`
- Целевой triple Rust: `x86_64-pc-windows-gnu`

Установка инструментов (Debian/Ubuntu):

```bash
sudo apt install -y mingw-w64 python3
rustup target add x86_64-pc-windows-gnu
```

macOS: `brew install mingw-w64 python`
Windows: достаточно `winget install LLVM.Mingw` (или MSYS2 пакет `mingw-w64-x86_64-gcc`) и `rustup target add x86_64-pc-windows-gnu`.

## 2. Структура проекта

```
Cargo.toml                  — crate-type = ["cdylib","rlib"], профиль release (LTO, strip)
.cargo/config.toml          — linker = x86_64-w64-mingw32-gcc для windows-gnu таргета
src/lib.rs                  — объявления модулей, экспорт VERSION
src/core.rs                 — чистая логика мыши (scroll/drag/toss), 18 юнит-тестов
src/config.rs               — парсер TOML-конфига
src/controller.rs           — оркестратор с состоянием перетаскивания
src/ffi.rs                  — C-экспорты mt_* для Java-адаптера Horizon
src/win_hook.rs             — опциональный Win32 low-level хук мыши (cfg(windows))
src/bin/demo.rs             — стендалон-демо без игры
host_adapter/MouseTweaksRs.java — JNI-обёртка / миксины на стороне хоста
config/mousetweaks_rs.toml  — настройки мода (попадают внутрь .hmod)
build_hmod.sh               — сборка DLL + упаковка пакета
build_hmod/mousetweaks_rs_v1_0_0/  — «этап» пакета: manifest.json, pack_icon.json,
                                      resource_packs/.../pack_icon.png
```

Файлы `manifest.json`, `pack_icon.json` и PNG-иконка лежат в каталоге этапа
`build_hmod/mousetweaks_rs_v1_0_0/` и берутся оттуда при упаковке; скрипт сборки
только подкладывает в них свежую нативную библиотеку и актуализирует путь
`entry_native`.

## 3. Команды сборки

Проверка логики (хостовые тесты):

```bash
cargo test --release        # ожидаем: 18 passed; 0 failed
```

Сборка Windows-DLL:

```bash
cargo build --release --target x86_64-pc-windows-gnu
# результат: target/x86_64-pc-windows-gnu/release/mousetweaks_rs.dll
```

Полная сборка пакета одним скриптом (таргет по умолчанию — Windows x64):

```bash
./build_hmod.sh                       # → MouseTweaks-RS_v1.0.0.hmod
./build_hmod.sh aarch64-linux-android # Android-вариант (имя libs автоматически .so)
```

Скрипт делает три шага: кросс-компиляция → копирование библиотеки в этап пакета
→ архивация этапа в ZIP с расширением `.hmod` (ZIP_DEFLATED).

## 4. Что внутри .hmod

```
manifest.json                                        Bedrock/Horizon манифест пака
pack_icon.json                                       метаданные: entry_native, config, icon
resource_packs/mtr_assets/textures/pack_icon.png     иконка пака
config/mousetweaks_rs.toml                           настройки
pe_packs/render_containers/mtr_native/mousetweaks_rs.dll  нативная библиотека (PE x64)
```

DLL должна экспортировать ровно эти символы (проверка: `objdump -p ... | grep mt_`):

```
mt_init  mt_scroll  mt_press  mt_drag_move  mt_release
mt_toss_attempt  mt_hud_text  mt_reload_config  mt_free_str
```

## 5. Установка в Horizon

Скопируйте `MouseTweaks-RS_v1.0.0.hmod` в каталог модов Horizon
(`.../Horizon/mods/`) и перезапустите клиент. Настройки правятся в
`config/mousetweaks_rs.toml` (параметры: `items_per_scroll`, `allow_left_dragging`,
`scroll_empty_slot_mode`, `disable_toss_enabled`).

## 5.1 Проверка целостности собранного пакета

```bash
# список и размеры содержимого
python3 -m zipfile -l MouseTweaks-RS_v1.0.0.hmod
# контрольная сумма (для сверки с авторской сборкой)
sha256sum MouseTweaks-RS_v1.0.0.hmod
```

Авторская сборка в этой среде: размер **594 815 байт**,
SHA-256 `679a10361693a96f0b64d025afe05005bb302457e74f0393c621e3c893508c27`.
Важно: байты самой DLL и всех остальных файлов внутри пакета воспроизводимы из
исходников один-в-один; различаться может только контейнер ZIP (метки времени),
поэтому итоговый SHA-256 пакета зависит от момента сборки — ориентируйтесь на
совпадение содержимого попарно (`python3 -m zipfile -e` + `diff -r`).

## 6. Частые проблемы

| Симптом | Причина / решение |
|---|---|
| `linker 'x86_64-w64-mingw32-gcc' not found` | не установлен mingw-w64 (см. п.1) |
| `error[E0463] can't find crate for std` | не добавлен таргет: `rustup target add x86_64-pc-windows-gnu` |
| Horizon не видит пак | проверьте `python3 -m zipfile -l MouseTweaks-RS_v1.0.0.hmod` — манифест должен быть в корне ZIP |
| Предупреждение `static_mut_refs` в win_hook.rs | безопасно игнорировать: единственный потоковый доступ к хуку идёт из окна сообщений Win32 |
