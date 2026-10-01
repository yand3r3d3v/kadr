# kadr: design

How kadr looks and sounds. What it does and how it is built is in [architecture.md](architecture.md).

kadr is a terminal form over ffmpeg. Its tagline: **the ffmpeg command, assembled as you watch.**

The screens are drawn on a design canvas, "kadr в терминале", 17 artboards. The code follows them, with the differences listed in [architecture.md](architecture.md#where-the-code-departs-from-the-canvas).

## The concept: a blueprint

The command comes together in front of you like a part on a blueprint. Everything else in the look stays quiet so that one thing can be seen: which field makes which piece of the command.

- Flags in the command are dimmed, values glow ochre.
- The field in focus brightens and underlines its own piece: on `crf` it is `-crf 23`, on `start` it is `-ss 00:00:38`.
- A choice that changes two places lights up both. The audio format changes the codec and the file extension.

This is how the flags get remembered without being memorised.

## Name and mark

- The name is **kadr** (кадр, "frame"), lowercase.
- The mark is a viewfinder frame: `⌜ kadr ⌟`. It is short enough for a window title, `--version` and a README.

## Palette

A colour's name says what it is for.

| Name | Hex | Role |
|---|---|---|
| Blueprint | `#0F2A4A` | Background, if the app paints its own. By default the terminal's background is used |
| Chalk | `#E8EEF5` | Main text |
| Ice | `#7FB8E6` | Frames, field labels, the part already done |
| Ochre | `#E3A72F` | Everything you can change: values, focus, cursor, the ends of a piece |
| Carmine | `#F0627A` | Error and risk |

Two more shades for secondary things: `#8A9BB0` for flags in the command and for hints, `#4A7BA6` for thin lines.

Carmine was lightened from `#D6455D`: on the blueprint background the error text had a contrast of 3.4 where 4.5 is needed.

Without truecolor the colours fold into ANSI-16: ice is cyan, ochre is yellow, carmine is red, chalk is the terminal's default text colour.

The first plan was "dark background and one bright accent". That is a template that would fit any ffmpeg tool, so it was dropped. Colour now carries a function: ochre means "you can change this", carmine means "careful".

## Layout

- Everything is aligned to the left edge; field labels form a column.
- Frames are thin, like lines on a drawing.
- Progress is a timeline ruler, `┣━━◆┈┈┫`, with a timecode. Not a block gauge.
- The active tab is underlined, not inverted.
- The keys for the current screen sit on the bottom row.

The marks the interface is built from:

| Mark | Meaning |
|---|---|
| `⌜ ⌟` | the viewfinder frame |
| `┣━━◆┈┈┫` | the timeline |
| `[` `]` | the ends of a chosen piece |
| `▎` `▏` | row focus and the text cursor |
| `✓` `✕` | done and error |

## Voice

Labels and tabs are lowercase. Messages are ordinary sentences. Short verbs, no arrows and no apologies. An error says what happened and how to fix it.

- Success: `✓ Done: lecture_04_small.mp4`, then before and after: `48 MB`, `12 MB`, `75% smaller`.
- Error: `lecture_04_small.mp4 already exists. Press o to overwrite it, or change the name.`

The interface is English first. A second language, Russian, is chosen inside the program: `?`, then `tab`. Operation and option names in the shell stay English in any language, and so does the ffmpeg command.

## Screens

| Screen | When |
|---|---|
| File picker | kadr starts without a file, or `f` |
| The form | one per operation |
| Value list | `space` on a field with many values, such as the preset |
| Run | ffmpeg is working: ruler, speed, size |
| Done | before and after sizes; for a cut, what was asked and what came out |
| File exists | before a run that would overwrite |
| ffmpeg stopped | a non-zero exit: its last lines, and what was cleaned up |
| Keys | `?` |
| ffmpeg output | `l` |
| Too small | the window is under 80×24 |

## Cut with a frame preview

Choosing the start and end of a piece blind is the hard part of cutting: you cannot see the video to find the moment.

The answer on the canvas is a frame of the video drawn in the terminal next to the fields, taken at the position being chosen. It works over ssh in terminals with graphics (kitty, WezTerm, iTerm2, ghostty, foot) and falls back to half-block characters everywhere else. The rest of the screen stays the same.

- The frame is on the left with its timecode under it; the fields are on the right.
- The timeline runs the full width below: `[` and `]` are the ends of the piece in ochre, `━` between them in ice, `◆` is the cursor.
- The command at the bottom is rebuilt on every move.

An earlier idea was to drive an external player, mpv, over its IPC socket. It was cheaper but does not work over ssh, so it was dropped.

### Fast and exact

- **fast** copies the stream. No re-encoding, but a copy can only start on a keyframe.
- **exact** re-encodes. The edge is clean and it takes longer.

The design assumed that a fast cut always starts earlier than asked. Testing showed it depends on the container; see [architecture.md](architecture.md#keyframes-and-fast-cuts). The principle stays: kadr says what will happen before the run and reports what did happen after it.
