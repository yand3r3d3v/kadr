<div align="center">

# ⌜ kadr ⌟

**Stop googling ffmpeg flags.**<br>
A terminal form that builds the ffmpeg command while you watch, then runs it.

[![License: MIT](https://img.shields.io/badge/license-MIT-7FB8E6.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-E3A72F.svg)](https://www.rust-lang.org)
[![Built with ratatui](https://img.shields.io/badge/built%20with-ratatui-7FB8E6.svg)](https://ratatui.rs)

**English** · [Русский](README.ru.md)

</div>

```
  ⌜ kadr ⌟                compress  cut  gif  audio  convert  speed  frame
  ──────────────────────────────────────────────────────────────────────────────
    file        lecture.mov                              02:10, 1920×1080, 48 MB
  ▎ crf         23▏                             18 (better) to 28 (smaller file)
    preset      medium
    resolution  source  720p  480p
    sound       keep  remove
    output      lecture_small.mp4

  ┌ command ───────────────────────────────────────────────────────────────────┐
  │ ffmpeg -i lecture.mov -c:v libx264 -crf 23 -preset medium -vf scale=-2:720 │
  │        -c:a aac -b:a 128k lecture_small.mp4                                │
  └────────────────────────────────────────────────────────────────────────────┘

  ┣┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┫  00:00 / 02:10

  enter run   ↑↓ field   ←→ value   tab operation   c copy   ? keys   esc quit
```

You know what you want: a smaller file, a piece from the middle, just the audio, a gif for the chat. You do not remember whether it was `-crf` or `-q:v`, or where `-ss` goes. kadr is a form for exactly that. Change a field and the command below changes with it. The field in focus lights up its own piece of the command, so after a few runs the flags stay in your head without any effort.

## Why kadr

- **The command on screen is the command that runs.** No hidden rewrites and no temporary names. Copy it with `c` and it works in your shell as is.
- **Every field shows its piece.** Focus `crf` and `-crf 23` lights up. Focus `resolution` and only the `720` inside `scale=-2:720` does.
- **You see the frame you are cutting at.** A frame of the video is drawn right in the terminal while you pick a time, even over ssh.
- **It does not lie about the result.** A fast cut starts on a keyframe. kadr tells you before you run it and measures the file afterwards.
- **It reads the file first.** No 1080p option for a 720p source, no audio flags for a silent clip.
- **It cleans up.** A cancelled or failed run removes its unfinished output, and ffmpeg never overwrites a file you did not agree to overwrite.
- **Works without the form.** `--print` for scripts and READMEs, `--run` when you already know the values.
- **Works in any terminal.** Truecolor and terminal graphics where there are some, plain ANSI and half blocks where there are not.

## Operations

| Operation | What it does | The command behind it |
|---|---|---|
| `compress` | Makes the file smaller: h264, optionally a smaller frame or no sound | `-c:v libx264 -crf 23 -preset medium -vf scale=-2:720` |
| `cut` | Takes a piece by time, `fast` (stream copy) or `exact` (re-encode) | `-ss 00:00:38 -to 00:01:12 -c copy` |
| `gif` | Turns a short fragment into a gif with its own palette | `-vf "fps=12,scale=480:-1:…palettegen…paletteuse"` |
| `audio` | Extracts the audio track, as mp3 or untouched | `-vn -c:a libmp3lame -q:a 2` |
| `convert` | Changes the container without re-encoding: mov to mp4 in a second | `-c copy` |
| `speed` | Speeds the video up or slows it down, sound included | `-vf setpts=PTS/2 -af atempo=2` |
| `frame` | Saves one frame as a png or jpg | `-ss 00:00:38 -frames:v 1` |

## Install

kadr needs `ffmpeg` and `ffprobe` in your `PATH`.

**From source**

```bash
cargo install --git https://github.com/yand3r3d3v/kadr
```

**Homebrew**

A formula that brings ffmpeg along is prepared and lands with the first tagged release. The steps are in [docs/release.md](docs/release.md).

## Usage

```bash
kadr                      # pick a file, then fill in the form
kadr lecture.mov          # open the form on a file
```

Give it an operation and options, and the form opens with the fields filled in:

```bash
kadr cut lecture.mov --from 0:38 --to 1:12
```

Or skip the form:

```bash
kadr compress lecture.mov --resolution 720p --print   # print the command, run nothing
kadr audio lecture.mov --run                          # run it right away
kadr gif lecture.mov --from 0:38 --length 6 --width 320 --run
kadr frame lecture.mov --at 1:05 --run
```

Option names follow the field labels, not the ffmpeg flags: `--resolution`, not `--vf`. `kadr <operation> --help` lists them.

## Picking a time

In `cut`, `gif` and `frame` a frame of the video sits next to the fields and follows the time you are changing.

```
  ▄▄▀▄▄▄▄▄▄                ▀▀▄▄            file        lecture.mov
      ▀▀▀▀▀▀▄▄▄▄▀▀           ▀▀▄▄        ▎ cursor      00:00:41▏
             ▀▀▀▄▄▄▀▀▀▀▄▄      ▀▀▄▄        start       00:00:38
                    ▄▄▄▀▀▀▄▄▄▄▀▀ ▀         end         00:01:12
                            ▀▀▄▄▄▀▀▀       length      00:00:34
  00:00:41          frame 1026 of 3250     mode        fast  exact

  ┣┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈[━◆━━━━━━━━━━━━━━━━━]┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┫
  00:00                00:38               01:12                           02:10
```

Move the `cursor` to look around, press `i` to make it the start and `o` to make it the end. Focus `start` or `end` and the frame shows that edge instead.

kadr asks the terminal which graphics it speaks: the kitty protocol, iTerm2 or sixel give a real picture, and every other terminal gets half blocks like the ones above. `KADR_PREVIEW=off` turns the preview off.

## Keys

| Key | Action |
|---|---|
| `tab` `⇧tab` | next and previous operation |
| `↑` `↓` | field |
| `←` `→` | change the value by one step |
| `⇧←` `⇧→` or `H` `L` | by a big step |
| digits | type a value, `enter` takes it |
| `space` | next value, or open the list |
| `,` `.` | one frame back and forward, in a time field |
| `i` `o` | in `cut`: the cursor becomes the start, the end |
| `backspace` | back to the default |
| `enter` | run the command |
| `c` | copy the command |
| `f` | another file |
| `l` | show the ffmpeg output |
| `?` | all the keys, and the language switch |
| `esc` | back; from the form, quit |

Letter keys also work in a Cyrillic keyboard layout.

## Configuration

`~/.config/kadr/config.toml` holds the interface language and the values a form starts with. Everything in it is optional.

```toml
lang = "ru"          # "en" or "ru"; switched from the keys page with tab

[compress]
crf = 26
preset = "slow"
resolution = "720p"

[audio]
format = "copy"      # "mp3" or "copy"

[gif]
fps = 15
width = 640
```

A default that does not fit a file, such as `720p` for a 480p clip, is skipped for that file. Options on the command line win over the config.

## What kadr adds to the command

Four service flags and nothing else: `-hide_banner -nostats -progress pipe:1` to read the progress, and `-n` so that ffmpeg refuses to overwrite a file unless the command on screen carries `-y`. None of them changes the result.

## A note on fast cuts

A stream copy can only start on a keyframe. What that means depends on the container:

- **mkv and others:** the result starts earlier than you asked, at the keyframe.
- **mp4 and mov:** ffmpeg keeps the extra frames in the file and hides them with an edit list, so players show exactly the piece you chose.

kadr shows which of the two you will get before you press enter. `exact` mode re-encodes and cuts clean in every container.

## Roadmap

- [x] `compress`, `cut`, `gif`, `audio`, `convert`, `speed`, `frame`
- [x] a frame of the video in the terminal while you pick a time
- [x] file picker, live command, run with progress and cancel
- [x] `--print` and `--run`
- [x] defaults in a config file
- [x] interface in English and Russian
- [ ] Homebrew tap
- [ ] joining several files into one

## How it is built

An operation is data: a list of fields and a pure function that turns their values into a command. Every piece of that command remembers the field it came from. The same list of pieces is drawn in the frame, handed to ffmpeg as `argv`, and printed by `--print`, so the three can never disagree.

More in [docs/architecture.md](docs/architecture.md); the look and the voice are in [docs/design.md](docs/design.md).

## Related projects

- [lazycut](https://github.com/ozemin/lazycut): a terminal video trimmer with a playing preview. kadr covers more operations and puts the command first.
- [LosslessCut](https://github.com/mifi/lossless-cut): the GUI for lossless cutting.

## Contributing

Issues and pull requests are welcome. `cargo test` runs the whole suite and needs no terminal; the tests cover command building, option parsing, the form's keys, screen rendering and the translation.

## License

[MIT](LICENSE)
