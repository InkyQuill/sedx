# Homebrew Formula for SedX
# Install with: brew install InkyQuill/sedx/sedx
# Or: brew install --formula ./contrib/homebrew/sedx.rb

class Sedx < Formula
  desc "Safe, modern replacement for GNU sed with automatic backups and rollback"
  homepage "https://github.com/InkyQuill/sedx"
  url "https://github.com/InkyQuill/sedx/archive/refs/tags/v0.2.6-alpha.tar.gz"
  sha256 "PLACEHOLDER_SHA256" # Update this on release
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
    # Install man page
    man1.install "man/sedx.1"
    # Generate and install shell completions
    bash_completion.install shell_output("bin/sedx --completions bash").chomp => "sedx"
    zsh_completion.install shell_output("bin/sedx --completions zsh").chomp => "_sedx"
    fish_completion.install shell_output("bin/sedx --completions fish").chomp => "sedx.fish"
  end

  test do
    (testpath/"test.txt").write("hello world")
    system bin/"sedx", "s/hello/HELLO/", testpath/"test.txt"
    assert_match "HELLO world", (testpath/"test.txt").read
  end
end
