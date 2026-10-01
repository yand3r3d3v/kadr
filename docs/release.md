# kadr: releases and Homebrew

The goal: `brew install yand3r3d3v/tap/kadr` installs kadr and ffmpeg with it.

The dependency is one line in the formula, `depends_on "ffmpeg"`; Homebrew installs it if it is not there yet. The formula template is [packaging/homebrew/kadr.rb](../packaging/homebrew/kadr.rb).

## What is missing before the first install

1. A version tag, and with it the source tarball the formula points at.
2. A tap: a separate repository named `homebrew-tap` (any name starting with `homebrew-` works).

## First release

1. Tag the version and push the tag:
   ```bash
   git tag v0.1.0 && git push origin v0.1.0
   ```
2. Take the checksum of the tarball GitHub builds for the tag:
   ```bash
   curl -sL https://github.com/yand3r3d3v/kadr/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256
   ```
3. Put it into the formula in place of `REPLACE_WITH_SHA256_OF_THE_TARBALL`.
4. Create the repository `yand3r3d3v/homebrew-tap` and put the formula at `Formula/kadr.rb`.
5. Check it:
   ```bash
   brew install yand3r3d3v/tap/kadr && brew test kadr
   ```

Before the first tag kadr can be installed straight from the branch with `brew install --HEAD yand3r3d3v/tap/kadr`. That needs only step 4; no checksum.

## Later releases

Raise `version` in `Cargo.toml`, tag, and update `url` and `sha256` in the formula. Once that gets tedious, steps 2 and 3 fit in one GitHub Actions workflow triggered by a tag.

## What the formula tests

The `test` block runs `kadr --version`, then makes a one-second clip and asks for `kadr compress in.mp4 --print`. That checks kadr itself and that the `ffprobe` from the dependency is found.

## Without Homebrew

```bash
cargo install --git https://github.com/yand3r3d3v/kadr
```

ffmpeg is installed separately in this case. If it is missing, kadr says so when it starts.
