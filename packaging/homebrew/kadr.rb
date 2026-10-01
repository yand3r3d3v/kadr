# Homebrew formula for kadr.
#
# This file is a template: it becomes installable once it sits in a tap
# (a GitHub repository named homebrew-<something>) and the two placeholders
# below point at a real release. The steps are in docs/release.md.
class Kadr < Formula
  desc "ffmpeg command, assembled as you watch"
  license "MIT"
  homepage "https://github.com/yand3r3d3v/kadr"
  url "https://github.com/yand3r3d3v/kadr/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_SHA256_OF_THE_TARBALL"
  head "https://github.com/yand3r3d3v/kadr.git", branch: "main"

  depends_on "rust" => :build
  # kadr builds the command; ffmpeg and ffprobe run it.
  depends_on "ffmpeg"

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/kadr --version")

    system Formula["ffmpeg"].opt_bin/"ffmpeg", "-v", "error", "-f", "lavfi",
           "-i", "testsrc2=size=320x240:rate=25:duration=1", "in.mp4"
    assert_equal "ffmpeg -i in.mp4 -c:v libx264 -crf 23 -preset medium in_small.mp4\n",
                 shell_output("#{bin}/kadr compress in.mp4 --print")
  end
end
