"""Generate the macOS tap formula from verified immutable GitHub release archives."""

import argparse
import os
from pathlib import Path
import tempfile

import distribution


def formula(version, archives):
    blocks = []
    for architecture, target in (("arm", "aarch64-apple-darwin"),
                                 ("intel", "x86_64-apple-darwin")):
        archive, url = archives[target]
        blocks.append(f'''    on_{architecture} do
      url "{url}"
      sha256 "{distribution.digest(distribution.regular(archive))}"
    end''')
    return '''require "json"

class FluidiseRelay < Formula
  desc "Agent-first Git hook manager with explicit plans and safe orchestration"
  homepage "https://github.com/Fluidise/relay"
''' + f'''  version "{version}"
  license "MIT"
  depends_on :macos
  depends_on "git"

  on_macos do
''' + "\n".join(blocks) + '''
  end

  def install
    bin.install "fluidise-relay"
    pkgshare.install "LICENSE", "USAGE.md", "THIRD-PARTY", "THIRD-PARTY-NOTICES.md"
  end

  test do
    assert_match "fluidise-relay #{version}", shell_output("#{bin}/fluidise-relay --version")
    ENV["GIT_CONFIG_GLOBAL"] = "/dev/null"
    ENV["GIT_CONFIG_SYSTEM"] = "/dev/null"
    system "git", "init", "--quiet", "--template="
    (testpath/"fluidise.toml").write <<~TOML
      [hooks]
      version = 1
      [[hooks.commit-msg]]
      id = "policy"
      program = "git"
      args = ["rev-parse", "--is-inside-work-tree"]
      modifies-message = false
    TOML
    (testpath/"message.txt").write "feat: package test\\n"
    %w[prepare install].each do |operation|
      extra = operation == "install" ? " --binary #{bin}/fluidise-relay" : ""
      plan = JSON.parse(shell_output("#{bin}/fluidise-relay inspect --operation #{operation}#{extra}"))
      system bin/"fluidise-relay", operation, "--plan", plan.fetch("plan").fetch("id"),
             * (operation == "install" ? ["--binary", (bin/"fluidise-relay").to_s] : [])
    end
    result = JSON.parse(shell_output("#{bin}/fluidise-relay run --file message.txt"))
    assert_equal "success", result.fetch("status")
    assert_equal "feat: package test\\n", (testpath/"message.txt").read
  end
end
'''


def rehearse(directory, destination):
    archives = list(directory.glob("*.tar.gz"))
    if len(archives) != 1:
        raise ValueError("expected one native archive")
    archive = archives[0].resolve()
    _, provenance, _ = distribution.read_archive(archive)
    distribution.native_target(provenance["target"])
    if not provenance["target"].endswith("apple-darwin"):
        raise ValueError("Homebrew rehearsal requires macOS")
    destination.mkdir(parents=True, exist_ok=False)
    fixture = destination / "fluidise-relay.rb"
    urls = {target: (archive, archive.as_uri()) for target in
            ("aarch64-apple-darwin", "x86_64-apple-darwin")}
    fixture.write_text("# NATIVE-ONLY TEST FIXTURE. NEVER PUBLISH.\n" +
                       formula(provenance["version"], urls), encoding="utf-8")
    _, binary = distribution.extract_verified(archive, destination / "unpacked")
    environment = os.environ.copy()
    environment.update({"HOMEBREW_NO_AUTO_UPDATE": "1", "HOMEBREW_NO_ANALYTICS": "1",
                        "HOMEBREW_NO_INSTALL_FROM_API": "1",
                        "HOMEBREW_CACHE": str(destination / "cache"),
                        "HOMEBREW_LOGS": str(destination / "logs")})
    distribution.run("brew", "install-bundler-gems", "--add-groups=formula_test", env=environment)
    distribution.run("brew", "ruby", str(distribution.ROOT / "scripts/test-homebrew.rb"),
                     str(fixture), str(binary.parent), str(destination / "prefix"),
                     provenance["version"], env=environment)
    distribution.smoke(destination / "prefix/bin/fluidise-relay", provenance["version"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    release = commands.add_parser("release")
    release.add_argument("--distribution", type=Path, required=True)
    release.add_argument("--tag", required=True)
    release.add_argument("--source", required=True)
    release.add_argument("--output", type=Path, required=True)
    test = commands.add_parser("rehearse")
    test.add_argument("--distribution", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "rehearse":
        with tempfile.TemporaryDirectory(prefix="relay-brew-") as directory:
            rehearse(args.distribution, Path(directory) / "rehearsal")
        return
    distribution.verify_release(args.distribution, args.tag, args.source)
    archives = {}
    for archive in args.distribution.glob("*.tar.gz"):
        _, provenance, _ = distribution.read_archive(archive)
        url = f"https://github.com/Fluidise/relay/releases/download/{args.tag}/{archive.name}"
        archives[provenance["target"]] = (archive, url)
    with args.output.open("x", encoding="utf-8") as output:
        output.write(formula(args.tag.removeprefix("v"), archives))


if __name__ == "__main__":
    main()
