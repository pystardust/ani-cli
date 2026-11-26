class AniCli < Formula
  desc "Cli tool to browse and play anime"
  homepage "https://github.com/pystardust/ani-cli"
  url "https://github.com/pystardust/ani-cli/archive/refs/tags/v5.2.tar.gz"
  sha256 "af2e3102d661d2eab0606f98f3b30c9e1a63d8e38adcaf5de6d3c0b191c9bd91"
  license "GPL-3.0"
  head "https://github.com/pystardust/ani-cli.git", branch: "master"

  depends_on "ffmpeg"
  depends_on "fzf"
  depends_on "grep"
  depends_on "yt-dlp"
  depends_on "mpv" => :recommended

  def install
    bin.install "ani-cli"
    man1.install "ani-cli.1"
  end

  def caveats
    on_macos do
      <<~EOS
        On macOS you can install IINA player instead of mpv for better experience:
          brew install --cask iina
      EOS
    end
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ani-cli --version")
    assert_match "No results found!", shell_output("#{bin}/ani-cli this-title-does-not-exist-for-sure 2>&1", 1)
  end
end
