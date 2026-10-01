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
  ⌜ kadr ⌟                compress  cut  audio
  ──────────────────────────────────────────────────────────────────────────────
    file        lecture.mov                              02:10, 1920×1080, 48 MB
  ▎ crf         23▏                             18 (better) to 28 (smaller file)
    preset      medium
    resolution  source  720p  480p
    output      lecture_small.mp4

  ┌ command ───────────────────────────────────────────────────────────────────┐
  │ ffmpeg -i lecture.mov -c:v libx264 -crf 23 -preset medium -vf scale=-2:720 │
  │        -c:a aac -b:a 128k lecture_small.mp4                                │
  └────────────────────────────────────────────────────────────────────────────┘

  ┣┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┫  00:00 / 02:10

  enter run   ↑↓ field   ←→ value   tab operation   c copy   ? keys   esc quit
```

You know what you want: a smaller file, a piece from the middle, just the audio. You do not remember whether it was `-crf` or `-q:v`, or where `-ss` goes. kadr is a form for exactly that. Change a field and the command below changes with it. The field in focus lights up its own piece of the command, so after a few runs the flags stay in your head without any effort.

## Why kadr

- **The command on screen is the command that runs.** No hidden rewrites and no temporary names. Copy it with `c` and it works in your shell as is.
- **Every field shows its piece.** Focus `crf` and `-crf 23` lights up. Focus `resolution` and only the `720` inside `scale=-2:720` does.
- **It does not lie about the result.** A fast cut starts on a keyframe. kadr tells you before you run it and measures the file afterwards.
- **It reads the file first.** No 1080p option for a 720p source, no audio flags for a silent clip.
- **It cleans up.** A cancelled or failed run removes its unfinished output, and ffmpeg never overwrites a file you did not agree to overwrite.
- **Works without the form.** `--print` for scripts and READMEs, `--run` when you already know the values.
- **Works over ssh, in any terminal.** Truecolor where there is one, plain ANSI where there is not.

## Operations

| Operation | What it does | The command behind it |
|---|---|---|
| `compress` | Makes the file smaller: h264, optionally a smaller frame | `-c:v libx264 -crf 23 -preset medium -vf scale=-2:720` |
| `cut` | Takes a piece by time, `fast` (stream copy) or `exact` (re-encode) | `-ss 00:00:38 -to 00:01:12 -c copy` |
| `audio` | Extracts the audio track, as mp3 or untouched | `-vn -c:a libmp3lame -q:a 2` |

More are on the way: see the [roadmap](#roadmap).

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
kadr cut lecture.mov --from 0:38 --to 1:12 --exact --run -y
```

Option names follow the field labels, not the ffmpeg flags: `--resolution`, not `--vf`. `kadr <operation> --help` lists them.

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
| `backspace` | back to the default |
| `enter` | run the command |
| `c` | copy the command |
| `f` | another file |
| `l` | show the ffmpeg output |
| `?` | all the keys |
| `esc` | back; from the form, quit |

Letter keys also work in a Cyrillic keyboard layout.

## What kadr adds to the command

Four service flags and nothing else: `-hide_banner -nostats -progress pipe:1` to read the progress, and `-n` so that ffmpeg refuses to overwrite a file unless the command on screen carries `-y`. None of them changes the result.

## A note on fast cuts

A stream copy can only start on a keyframe. What that means depends on the container:

- **mkv and others:** the result starts earlier than you asked, at the keyframe.
- **mp4 and mov:** ffmpeg keeps the extra frames in the file and hides them with an edit list, so players show exactly the piece you chose.

kadr shows which of the two you will get before you press enter. `exact` mode re-encodes and cuts clean in every container.

## Roadmap

- [x] `compress`, `cut`, `audio`
- [x] file picker, live command, run with progress and cancel
- [x] `--print` and `--run`
- [ ] a frame of the video right in the terminal while you pick a time
- [ ] `gif`, `convert` (change the container), `speed`, `frame` (save a still)
- [ ] defaults in a config file
- [ ] interface language switch (English, Russian)
- [ ] Homebrew tap

## How it is built

An operation is data: a list of fields and a pure function that turns their values into a command. Every piece of that command remembers the field it came from. The same list of pieces is drawn in the frame, handed to ffmpeg as `argv`, and printed by `--print`, so the three can never disagree.

More in [docs/architecture.md](docs/architecture.md); the look and the voice are in [docs/design.md](docs/design.md).

## Related projects

- [lazycut](https://github.com/ozemin/lazycut): a terminal video trimmer with a playing preview. kadr covers more operations and puts the command first.
- [LosslessCut](https://github.com/mifi/lossless-cut): the GUI for lossless cutting.

## Contributing

Issues and pull requests are welcome. `cargo test` runs the whole suite and needs no terminal; the tests cover command building, option parsing, the form's keys and screen rendering.

## License

[MIT](LICENSE)
