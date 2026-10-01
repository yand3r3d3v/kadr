# kadr: выпуск и Homebrew

Цель: `brew install OWNER/tap/kadr` ставит kadr и вместе с ним ffmpeg.

Зависимость от ffmpeg задаётся в формуле строкой `depends_on "ffmpeg"`. Homebrew ставит её сам, если её ещё нет. Шаблон формулы лежит в [packaging/homebrew/kadr.rb](../packaging/homebrew/kadr.rb).

## Чего не хватает до первой установки

1. Репозиторий на GitHub. Сейчас он только локальный.
2. Лицензия. Без файла `LICENSE` формулу в свой tap положить можно, но так не принято. Какую лицензию взять, решаешь ты.
3. Тег версии и архив с исходниками, на который будет ссылаться формула.
4. Tap: отдельный репозиторий с именем `homebrew-tap` (или любым другим, начинающимся с `homebrew-`).

## Шаги первого выпуска

1. Отправить код на GitHub, поставить тег:
   ```bash
   git tag v0.1.0 && git push origin main v0.1.0
   ```
2. Посчитать сумму архива, который GitHub собирает для тега:
   ```bash
   curl -sL https://github.com/OWNER/kadr/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256
   ```
3. В формуле заменить `OWNER` и `REPLACE_WITH_SHA256_OF_THE_TARBALL`.
4. Создать репозиторий `OWNER/homebrew-tap`, положить формулу в `Formula/kadr.rb`.
5. Проверить:
   ```bash
   brew install OWNER/tap/kadr && brew test kadr
   ```

До первого тега можно ставить прямо из ветки: `brew install --HEAD OWNER/tap/kadr`. Для этого достаточно шагов 3 и 4, сумма архива не нужна.

## Следующие выпуски

Поднять `version` в `Cargo.toml`, поставить тег, обновить в формуле `url` и `sha256`. Когда надоест делать это руками, шаги 2 и 3 автоматизируются одним workflow в GitHub Actions на событие тега.

## Что формула проверяет

Блок `test` запускает `kadr --version` и собирает команду для минутного тестового ролика через `kadr compress in.mp4 --print`. Это проверяет сразу и kadr, и то, что ffprobe из зависимости находится.

## Без Homebrew

```bash
cargo install --path .
```

ffmpeg в этом случае ставится отдельно. Если его нет, kadr скажет об этом при запуске.
