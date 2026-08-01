class AniCli < Formula
  desc "Cli tool to browse and play anime"
  homepage "https://github.com/pystardust/ani-cli"
  url "https://github.com/pystardust/ani-cli/archive/refs/tags/v5.0.tar.gz"
  sha256 "e4703d2f563eee27ea16d92f8e77e3f8a1f07ba8b2433598c3a1ce642841c35c"
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
    <<~EOS
      On macOS you can install IINA player instead of mpv for better experience:
        brew install --cask iina
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ani-cli --version")
    assert_match "No results found!", shell_output("#{bin}/ani-cli this-title-does-not-exist-for-sure 2>&1", 1)
  end
end
