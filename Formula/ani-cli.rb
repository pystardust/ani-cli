class AniCli < Formula
  desc "Cli tool to browse and play anime"
  homepage "https://github.com/pystardust/ani-cli"
  url "https://github.com/pystardust/ani-cli/archive/refs/tags/v5.1.tar.gz"
  sha256 "f7aa33b547b36ddb7e00ac23216b09ee08212161936d3b9e247bcce95fd2e041"
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
