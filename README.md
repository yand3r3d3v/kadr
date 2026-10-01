# ⌜ kadr ⌟

The ffmpeg command, assembled as you watch.

kadr is a terminal form over ffmpeg. Pick a file and an operation, change a few fields, and the exact ffmpeg command builds at the bottom of the screen. The field in focus lights up its own piece of the command, so the flags stick without memorising them. Enter runs it.

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
```

## Operations

| | |
|---|---|
| `compress` | make the file smaller: h264, with an optional smaller frame |
| `cut` | take a piece by time, by stream copy (`fast`) or re-encoded (`exact`) |
| `audio` | extract the audio track, as mp3 or untouched |

## Install

kadr needs `ffmpeg` and `ffprobe` in `PATH`.

```bash
cargo install --path .
```

A Homebrew formula that brings ffmpeg along is prepared; see [docs/release.md](docs/release.md).

## Use

```bash
kadr                      # pick a file, then fill the form
kadr lecture.mov          # open the form on a file
kadr cut lecture.mov --from 0:38 --to 1:12          # the form, fields filled in
kadr compress lecture.mov --resolution 720p --print # print the command, run nothing
kadr audio lecture.mov --run                        # run without the form
```

`-y` overwrites an existing output. Without it kadr asks in the form and refuses with `--run`.

Press `?` in the form for the keys.

## What kadr runs

The command on screen is the command that runs. kadr adds only `-hide_banner -nostats -progress pipe:1` to read the progress, and `-n` so ffmpeg never overwrites a file unless the command shows `-y`.

A cancelled or failed run removes its unfinished output.

## Docs

The design and the plan are in [docs/](docs/), in Russian.
