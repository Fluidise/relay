# Exercise the existing recipe using the real Homebrew DSL, but only a temporary
# prefix. This is not `brew install`, tap registration, linking, or a full brew audit.
require "formulary"
require "formula_assertions"
require "uri"

formula_path, archive_root, temporary_prefix, version = ARGV
# Load only this explicitly supplied test fixture through Homebrew's contents
# loader. Do not register a tap or change Homebrew's local-package trust policy.
path = Pathname(formula_path)
# A nil tap makes Formula.initialize discover homebrew/core and can clone it.
# Supply an in-memory namespace instead; Tap.fetch does not install/register it.
formula = Formulary.from_contents(
  "fluidise-relay", path, path.read, tap: Tap.fetch("fluidise/candidate-rehearsal")
)
formula.extend(Homebrew::Assertions)
raise "wrong version" unless formula.version.to_s == version
raise "fixture must use a local archive" unless formula.stable.url.start_with?("file://")
archive = Pathname(URI::DEFAULT_PARSER.unescape(URI(formula.stable.url).path))
raise "wrong archive checksum" unless Digest::SHA256.file(archive).hexdigest == formula.stable.checksum.to_s
prefix = Pathname(temporary_prefix)
raise "prefix already exists" if prefix.exist? || prefix.symlink?
formula.define_singleton_method(:prefix) { |*_args| prefix }
Dir.chdir(archive_root) { formula.install }
Dir.mktmpdir("relay-formula-test-") do |work|
  formula.define_singleton_method(:testpath) { Pathname(work) }
  Dir.chdir(work) { formula.test }
end
%w[LICENSE USAGE.md THIRD-PARTY THIRD-PARTY-NOTICES.md].each do |name|
  raise "missing installed notices: #{name}" unless (formula.pkgshare/name).exist?
end
