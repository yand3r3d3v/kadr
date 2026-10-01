# kadr: architecture

What kadr does, how it is put together, and what comes next. The look and the voice are in [design.md](design.md); releasing and Homebrew are in [release.md](release.md).

## What kadr is

A form over ffmpeg. You pick a file and an operation, change a few fields, see the finished ffmpeg command below them, and run it without leaving the form.

Three promises shape everything else:

1. **The command on screen is the command that runs.** No hidden changes to the output, no temporary names. The one exception is named below, under [service flags](#service-flags).
2. **Every field shows its piece of the command.** Focus on a field lights up its tokens.
3. **kadr does not lie about the result.** If a cut starts somewhere other than where you asked, that is said before the run and after it.

What kadr is not: a video editor, a player, a batch converter, or a wrapper around every ffmpeg flag.

## Status

All three planned stages are built, except joining files:

- seven operations: `compress`, `cut`, `gif`, `audio`, `convert`, `speed`, `frame`;
- the file picker, value lists, the run screen with cancel, the done, file-exists and ffmpeg-stopped screens, the keys page, the ffmpeg output page;
- a frame of the video next to the fields in `cut`, `gif` and `frame`;
- `--print` and `--run`;
- a config file with defaults;
- the interface in English and Russian, switched inside the program.

Verified by 75 tests (command building, option parsing, the form's keys, screen rendering, the translation), by driving the form through a pseudo-terminal on a real clip, and by `--run` on every operation. Built against ffmpeg 9.0.2 and cargo 1.98.

One thing is not verified: the frame preview through real terminal graphics. The pseudo-terminal used for testing speaks no graphics protocol, so only the half-block path has been seen working. The kitty, iTerm2 and sixel paths go through the same code up to the last call into ratatui-image.

## Prior art

| Project | What it is | What kadr takes | What it lacks |
|---|---|---|---|
| [lazycut](https://github.com/ozemin/lazycut) (Go) | A TUI for trimming only: a preview through chafa, `i`/`o` marks, export | Frame stepping with `,` `.`, `?` for help, a subcommand that works without the TUI | Other operations. It needs chafa as a system library. The main complaint in its Hacker News thread: `-ss -i -t` cuts are inexact and the tool does not say so |
| [ffmpeg-command-builder](https://github.com/0xelitesystem/ffmpeg-command-builder) | One HTML page: 32 tasks, a command that updates live, every flag explained | The task list as a guide to what people need. A hint next to each field | It does not run ffmpeg and does not see the file |
| LosslessCut | A GUI for cutting without re-encoding | Honesty about keyframes | Not a terminal |
| A dozen `ffmpeg-tui` repositories | Forms over ffmpeg | Nothing: they are lists of flags, not of tasks | The link between a field and the command |

The niche "a few frequent tasks, a live command, an honest result" was open in the terminal.

## Operations by demand

There are no measurements behind this order. It comes from what ffmpeg cheat sheets and FAQs put first and from the task list of ffmpeg-command-builder. It is an estimate.

| # | Task | How often | Cost for kadr | Stage |
|---|---|---|---|---|
| 1 | Compress, including a smaller frame and conversion to mp4 | All the time | Low | 1 |
| 2 | Cut a piece | All the time | Medium; high with a frame preview | 1 without the frame, 2 with it |
| 3 | Extract the audio | Often | Low | 1 |
| 4 | A gif from a fragment | Often | Low, the timeline exists | 2 |
| 5 | Change the container without re-encoding (mov to mp4) | Often | Low | 2 |
| 6 | Remove the audio | Sometimes | One field in `compress` | 2 |
| 7 | Speed up or slow down | Sometimes | Low | 3 |
| 8 | Save one frame as an image | Sometimes | Low once the preview exists | 3 |
| 9 | Join several files | Sometimes | High: a list of files, compatibility checks | Not built, not designed yet |
| 10 | Burn in subtitles, crop, rotate | Rarely | Medium | Maybe never |

Rows 1 to 8 are built. There is no separate "resize" operation: it is the `resolution` field of `compress`.

## Command line

Operations, options and help are English. The form has a second language, Russian, switched inside the program; operation and option names in the shell stay English in any language.

```
kadr [FILE]                       open the form; without a file, the picker first
kadr <OPERATION> FILE [OPTIONS]   open the form on that operation, fields filled in

compress  --crf N  --preset NAME  --resolution 720p  --no-audio   -o OUT
cut       --from T --to T         --exact                          -o OUT
gif       --from T --length S     --fps N  --width N               -o OUT
audio     --format mp3|copy       --quality N                      -o OUT
convert   --container mp4|mkv|mov                                  -o OUT
speed     --speed 0.5|0.75|1.25|1.5|2                              -o OUT
frame     --at T                  --format png|jpg                 -o OUT

--print     build the command, print it, exit
--run       run right away, without the form
-y, --yes   overwrite without asking
```

Option names follow the form's field labels, not the ffmpeg flags: `--resolution`, not `--vf`. Time is accepted as `38`, `0:38`, `00:00:38` or `38.5`.

`resolution` is the frame height in pixels with the width following automatically. It is called that, and written `720p`, because that is what people call it. In the command it is `-vf scale=-2:720`, and focus on the field lights up the `720`.

## States

```
          ┌──────────┐  file chosen  ┌────────┐
 start ──▶│  picker  │──────────────▶│  form  │◀──────────────┐
          │          │◀──────────────│        │               │
          └──────────┘       f       └───┬────┘               │
                                         │ enter              │
                                output exists? ──yes──▶ "file exists" ── esc ──┤
                                         │ no, or o pressed                    │
                                         ▼                                     │
                                    ┌─────────┐  esc: cancel                   │
                                    │   run   │────────────────────────────────┤
                                    └──┬───┬──┘                                │
                             code 0    │   │  code not 0                       │
                                       ▼   ▼                                   │
                                  "done"  "ffmpeg stopped" ── esc ─────────────┘
```

Over any state there can be: a field's value list, the keys page, the ffmpeg output, the "too small" page.

Started with a file, kadr skips the picker. Started with `--print` or `--run`, there is no form at all.

## Structure

One executable, no async runtime. The main thread draws and handles events; worker threads send events into a single `mpsc` channel.

```
src/
  main.rs      argument parsing, choice of mode: form, --print, --run
  cli.rs       clap; options become initial field values
  probe.rs     ffprobe -of json → MediaInfo; keyframe lookup
  op/
    mod.rs     operation kinds, FieldSpec, command parts, the builder
    compress.rs, cut.rs, gif.rs, audio.rs, convert.rs, speed.rs, frame.rs
               one file per operation: fields, checks, build()
  command.rs   the command frame: wrapping, lighting up the focused pieces
  run.rs       starting ffmpeg, parsing -progress, cancel, the stderr tail
  preview.rs   the frame of the video: a worker thread and a cache
  picker.rs    the file list
  app.rs       state, event handling, transitions
  ui.rs        drawing
  plain.rs     --print and --run
  config.rs    ~/.config/kadr/config.toml
  text.rs      the Russian interface
  theme.rs     the palette, truecolor or ANSI-16
  util.rs      time, size and shell formatting
```

### The core: an operation as data

The whole value of the program is that a field and a piece of the command are linked. So an operation is data, not drawing code:

```rust
struct FieldSpec { id: FieldId, label: &'static str, kind: Kind, value: String, hint: &'static str, enabled: bool }

enum Kind {
    File,
    Int { min: i64, max: i64, soft: (i64, i64), step: i64, big: i64 },
    Choice { items: Vec<Choice>, selected: usize, inline: bool },
    Time,
    Text,
    Info,
}

struct Part { text: String, role: Role, field: Option<FieldId> }
enum Role { Program, Flag, Fixed, Value, Output }

struct Arg { parts: Vec<Part>, is_output: bool }   // one argv entry
```

`build(op, values, info)` is a pure function from field values and file facts to a `Vec<Arg>`. One argument can hold several parts: in `scale=-2:720` only `720` belongs to a field. From that one list come:

- the command frame (the role sets the colour, `field` sets the highlight on focus);
- `argv` for the run (part texts joined, no shell involved);
- the string for `--print` and for copying (quoted where an argument needs it).

Because all three come from the same list, they cannot disagree.

One choice can produce several parts. The audio format `as is` changes both `-c:a copy` and the `.m4a` extension; both carry the same `FieldId` and light up together.

The core is tested with snapshots: field values in, command string out. No terminal is needed.

### Service flags

At run time kadr adds `-hide_banner -nostats -progress pipe:1`, and `-n` when the command has no `-y`. They do not change the result and are not shown in the frame, where they would bury the command in noise. This is the only gap between what is shown and what is run, and it is stated on the keys page and in the README.

`-y` appears in the command only after the user pressed `o` on the "file exists" screen, or passed `-y` on the command line.

### Running

- `run.rs` spawns ffmpeg and reads `stdout` line by line: `out_time_us`, `fps`, `speed`, `total_size`, `progress=`.
- `stderr` goes into a ring buffer of the last 200 lines, for the "ffmpeg stopped" screen and for the `l` key.
- Progress is counted against the length of the result, not of the input: for a cut that is the length of the piece.
- Cancel writes `q` to ffmpeg's stdin, so it exits the way it would for a person; after two seconds a process still running is killed. This needs no signal handling and no libc.
- An unfinished output is removed, because kadr created it. If the file existed before and was being overwritten, it is already gone; the "file exists" screen is the warning.
- The terminal is restored on exit and on panic.

### Probing

`ffprobe -v error -show_format -show_streams -of json`, once when a file is chosen. From it: duration, size, codecs, frame size, frame rate, whether there is video and audio. The fields depend on it:

- no audio: the `audio` tab is unavailable, and `compress` drops `-c:a aac -b:a 128k`;
- no video: only `audio` is available;
- a 720p source: `1080p` is not offered. kadr never suggests scaling up.

Cover art in an mp3 is a video stream to ffprobe; kadr does not count it.

## Operations

### compress

```
ffmpeg -i IN -c:v libx264 -crf 23 -preset medium -vf scale=-2:720 -c:a aac -b:a 128k IN_small.mp4
```

- `crf` steps by 1; arrows and typing allow 0 to 51, and a value outside 18 to 28 turns carmine.
- `preset` opens as a list with a line about each speed.
- `resolution` is `source`, `1080p`, `720p` or `480p`. `source` removes `-vf` from the command altogether.
- The size of the result cannot be predicted from `crf`, so kadr does not promise an estimate. The run screen shows the size so far.

### cut

```
fast:   ffmpeg -ss A -to B -i IN -c copy IN_cut.EXT
exact:  ffmpeg -ss A -to B -i IN -c:v libx264 -crf 18 -c:a aac IN_cut.mp4
```

Checks: the end is after the start, and both are inside the file.

### Keyframes and fast cuts

A stream copy can only begin at a keyframe. The design assumed this always makes the result start earlier than asked. Testing on ffmpeg 9 showed that it depends on the container:

- **mkv and others:** the file does start at the keyframe. Asked for 7 to 12 s with keyframes every 5 s, the result is 7 seconds long and starts at 5 s.
- **mp4 and mov:** ffmpeg writes the frames from the keyframe on, plus an edit list that hides them. Players show exactly the 5 seconds chosen. The extra frames stay in the file and are visible only to software that ignores edit lists.

So the note under the fields depends on the extension of the output: `starts at 00:00:05, the keyframe before it` for mkv, `keeps 2.0 s from the keyframe before it, hidden from players` for mp4 and mov. The keyframe is looked up in the background with `ffprobe -skip_frame nokey -read_intervals` around the chosen start, so a two-hour file is not scanned whole, and only after the start has stopped changing for a quarter of a second.

The done screen assumes nothing: it measures the length of the file that came out and reports a shift only if there is one, with `t` to cut again in exact mode.

### audio

```
mp3:    ffmpeg -i IN -vn -c:a libmp3lame -q:a 2 IN.mp3
as is:  ffmpeg -i IN -vn -c:a copy IN.m4a        the extension follows the codec
```

### gif

```
ffmpeg -ss A -t D -i IN -vf "fps=12,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" -loop 0 IN.gif
```

The filter builds a palette from the piece itself, which is what makes a gif look right. Of that whole string only `12` and `480` belong to fields, and that is all the highlight shows. The frame wraps the filter after a `,` or `;` when it does not fit on a line.

The length is capped at 60 seconds and turns carmine past 15: a long gif gets heavy fast. The width never exceeds the source.

### convert

```
ffmpeg -i IN -c copy IN.mp4
```

Changes the container and nothing else. The containers on offer are mp4, mkv and mov, minus the one the file is already in. A codec that the chosen container cannot hold makes ffmpeg stop, and the "ffmpeg stopped" screen shows why.

### speed

```
ffmpeg -i IN -vf setpts=PTS/2 -af atempo=2 IN_2x.mp4
```

One choice sets the factor in three places: the video filter, the audio filter and the file name. Without an audio track `-af` is left out. Progress is counted against the new length.

### frame

```
ffmpeg -ss T -i IN -frames:v 1 IN_00-00-38.png
```

The name carries the time, so frames taken one after another do not overwrite each other. `jpg` adds `-q:v 2`.

## The frame preview

In `cut`, `gif` and `frame` a frame of the video is drawn to the left of the fields. It shows the time being chosen: the field in focus if it is a time, otherwise the cursor.

- The frame comes from `ffmpeg -ss T -i IN -frames:v 1 -vf scale=W:-2 -f image2pipe -vcodec png -`, scaled to the pixels the preview will fill and no more.
- One worker thread serves the requests, "latest wins": while an arrow key is held, the frames in between are never fetched. The worker also encodes the frame for the terminal, so the drawing thread only blits.
- The last 48 frames are cached, keyed by time and size, so going back is instant.
- Drawing is [ratatui-image](https://docs.rs/ratatui-image). At start it asks the terminal which protocol it speaks (kitty, iTerm2, sixel) and falls back to half blocks. That query reads the answer from stdin, so it runs before the input thread starts. It needs no system library, unlike chafa.
- The preview takes what the window can spare: up to 12 rows, none at all under 22 rows. The form works the same without it.
- `KADR_PREVIEW=off` switches it off.

In `cut` the form has a `cursor` field: a time that is not part of the command. Move it to look around, then `i` makes it the start and `o` the end. `start` and `cursor` are shared between tabs, so a moment found in `cut` is there in `gif` and `frame`.

## Configuration and language

`~/.config/kadr/config.toml` (or under `$XDG_CONFIG_HOME`) holds `lang` and default field values per operation. Defaults are applied to every file that is opened, leniently: a value that does not fit a file is skipped for it. Command line options are applied after them, strictly: a value that does not fit is an error. `backspace` on a field returns to the configured default.

The Russian interface is a table from the English text to the Russian one, in `text.rs`. The English string in the code is the key, and a string without an entry is shown as it is. The ffmpeg command, option names and file names never go through it. `tab` on the keys page switches the language and writes `lang` back to the config, leaving the rest of the file, comments included, as it was.

In a narrow window a longer language must not push `esc` off the key bar, so the bar drops the keys the form makes obvious (`copy`, then `field`, then `value`) until it fits.

## Where the code departs from the canvas

- **The form has an `output` field.** The "file exists" screen offered to change the name and there was nowhere to change it.
- **The cursor is a field.** On the canvas the arrows move a cursor on the timeline. In the code `cursor` is a row of the form like `start` and `end`, and the arrows move whichever of the three is in focus.
- **The preview is smaller than drawn** in a small window, and absent in a very small one.
- **Seven tabs, not four**, and `convert`, `speed` and `frame` have no artboards.
- **`--help` is clap's standard help**, not the layout drawn on the canvas.
- **The picker has no path completion on `tab`.** Folders open with `enter`.
- **Operations switch with `tab`, not `←→`.** The arrows belong to the field: a step of the value.
- **Letter keys work in a Cyrillic layout** (`с` is `c`, `д` is `l`). Otherwise half the keys are dead with that layout on.

## What is left

- **Joining files.** It needs a list of files, which the one-file form does not have, and checks that the files are compatible. Not designed yet.
- **A Homebrew tap** and the first tagged release; see [release.md](release.md).
- **Seeing the preview in terminals with graphics.** See the note under [Status](#status).

## Decisions on the open questions

| Question | Decision |
|---|---|
| mpv or a frame in the terminal | A frame in the terminal. mpv does not work over ssh |
| Show the start shift before the run | Yes, by looking up the keyframe in the background |
| Which operations after compress and cut | audio in stage 1, gif in stage 2; resize is a field of compress |
| A mode without the TUI | Yes: `--print` and `--run`, plus subcommands with options |
| Presets in TOML | Built as defaults in a config file |
| Language | English first; Russian as a switch on the keys page |
| Distribution | Homebrew with ffmpeg as a dependency; see [release.md](release.md) |
