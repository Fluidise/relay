"""Packaging regression tests: isolated from publication and global installs."""

import io
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import distribution
import homebrew


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)

    def candidate(self):
        name = "fluidise-relay"
        provenance = {"target": "aarch64-apple-darwin", "archive_root": "candidate",
                      "version": "0.1.0", "binary_sha256": distribution.digest(b"executable")}
        files = {name: b"executable", "LICENSE": b"MIT", "USAGE.md": b"usage",
                 "THIRD-PARTY-NOTICES.md": b"runtime notices",
                 "THIRD-PARTY/nested/LICENSE": b"terms",
                 "PROVENANCE.json": json.dumps(provenance).encode()}
        self.inventory(files)
        return files, name

    @staticmethod
    def inventory(files):
        files.pop("FILES.sha256", None)
        files["FILES.sha256"] = "".join(
            f"{distribution.digest(data)}  {name}\n" for name, data in sorted(files.items())
        ).encode()

    def archive(self):
        files, binary = self.candidate()
        path = self.base / "candidate.tar.gz"
        distribution.write_archive(path, "candidate", files, binary)
        return path, files, binary

    def test_roundtrip_reproducibility_and_no_overwrite(self):
        archive, files, binary = self.archive()
        other = self.base / "second.tar.gz"
        distribution.write_archive(other, "candidate", files, binary)
        self.assertEqual(archive.read_bytes(), other.read_bytes())
        with self.assertRaises(FileExistsError):
            distribution.write_archive(other, "candidate", files, binary)
        actual, _, actual_binary = distribution.read_archive(archive)
        self.assertEqual(actual, files)
        self.assertEqual(actual_binary, binary)
        destination = self.base / "extracted"
        _, installed = distribution.extract_verified(archive, destination)
        self.assertEqual(installed.read_bytes(), b"executable")
        self.assertEqual(installed.stat().st_mode & 0o777, 0o755)
        with self.assertRaises(FileExistsError):
            distribution.extract_verified(archive, destination)
        with tarfile.open(archive) as stream:
            self.assertTrue(all(i.mtime == 0 and i.uid == 0 and i.gid == 0 for i in stream))

    def release_archives(self, directory, change=None, omit=None):
        directory.mkdir()
        for target in distribution.TARGETS:
            if target == omit:
                continue
            files, binary = self.candidate()
            provenance = json.loads(files["PROVENANCE.json"])
            provenance.update(target=target, source_commit="a" * 40, working_tree_dirty=False,
                              archive_root=target, public_distribution_ready=True,
                              publication_blockers=[], publisher_signing_policy="unsigned",
                              cargo_lock_sha256=distribution.digest(
                                  distribution.regular(distribution.ROOT / "Cargo.lock")))
            if change:
                change(provenance)
            files["PROVENANCE.json"] = json.dumps(provenance).encode()
            self.inventory(files)
            distribution.write_archive(directory / (target + ".tar.gz"), target, files, binary)

    def test_tampered_payload_rejected_before_extraction(self):
        files, binary = self.candidate()
        files[binary] = b"tampered"
        path = self.base / "bad.tar.gz"
        distribution.write_archive(path, "candidate", files, binary)
        destination = self.base / "must-not-exist"
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            distribution.extract_verified(path, destination)
        self.assertFalse(destination.exists())

    def test_tar_rejects_unsafe_entries(self):
        cases = ["../escape", "/absolute", "root/../escape", "root//file", "root/./file",
                 "root\\escape", "C:/escape", "root/file:stream"]
        for index, name in enumerate(cases):
            path = self.base / f"unsafe-{index}.tar.gz"
            with tarfile.open(path, "w:gz") as stream:
                info = tarfile.TarInfo(name)
                info.size = 1
                stream.addfile(info, io.BytesIO(b"x"))
            with self.subTest(name=name), self.assertRaises(ValueError):
                distribution.read_archive(path)

    def test_tar_rejects_links_and_special_files(self):
        for index, kind in enumerate((tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.DIRTYPE, tarfile.FIFOTYPE)):
            path = self.base / f"link-{index}.tar.gz"
            with tarfile.open(path, "w:gz") as stream:
                info = tarfile.TarInfo("root/link")
                info.type = kind
                info.linkname = "../../escape"
                stream.addfile(info)
            with self.assertRaises(ValueError):
                distribution.read_archive(path)

    def test_missing_license_payload_rejected(self):
        files, binary = self.candidate()
        del files["LICENSE"]
        path = self.base / "missing.tar.gz"
        distribution.write_archive(path, "candidate", files, binary)
        with self.assertRaisesRegex(ValueError, "required archive payload"):
            distribution.read_archive(path)

    def test_case_collisions_and_multiple_roots_rejected(self):
        for names in (("root/file", "root/FILE"), ("first/file", "second/file")):
            path = self.base / "names.tar.gz"
            with tarfile.open(path, "w:gz") as stream:
                for name in names:
                    info = tarfile.TarInfo(name)
                    stream.addfile(info, io.BytesIO(b""))
            with self.assertRaises(ValueError):
                distribution.read_archive(path)

    def test_expansion_limit(self):
        archive, _, _ = self.archive()
        with patch.object(distribution, "MAX_ARCHIVE_BYTES", 1):
            with self.assertRaisesRegex(ValueError, "limits"):
                distribution.read_archive(archive)

    def test_native_runner_required(self):
        with patch.object(distribution.platform, "system", return_value="Linux"), \
                patch.object(distribution.platform, "machine", return_value="x86_64"):
            distribution.native_target("x86_64-unknown-linux-musl")
            with self.assertRaises(ValueError):
                distribution.native_target("aarch64-unknown-linux-musl")

    def test_regular_rejects_symlinks(self):
        if distribution.platform.system() == "Windows":
            self.skipTest("symlink creation requires Windows developer mode")
        source = self.base / "ordinary"
        source.write_text("text")
        link = self.base / "link"
        link.symlink_to(source)
        with self.assertRaises(ValueError):
            distribution.regular(link)

    def test_missing_cargo_license_fails_closed(self):
        package = self.base / "dependency"
        package.mkdir()
        metadata = {"resolve": {"root": "root", "nodes": [{"id": "dependency"}]},
                    "packages": [{"id": "dependency", "name": "dependency", "version": "1.0.0",
                                  "manifest_path": str(package / "Cargo.toml")}]}
        with self.assertRaisesRegex(ValueError, "missing license text"):
            distribution.notices(metadata, self.base / "sysroot")

    def test_runtime_notice_inventory_and_compiler_identity(self):
        source = json.loads((distribution.ROOT / "licenses/runtime/common/SOURCES.json").read_text())
        compiler = f"rustc {source['rust_release']}\ncommit-hash: {source['rust_commit']}\n"
        for target in distribution.TARGETS:
            with self.subTest(target=target):
                files = distribution.runtime_notices(target, compiler)
                self.assertIn("THIRD-PARTY/runtime/common/compiler-builtins-LICENSE.txt", files)
                self.assertIn("THIRD-PARTY/runtime/common/libunwind-LICENSE.txt", files)
                self.assertEqual("THIRD-PARTY/runtime/musl-1.2.5/COPYRIGHT" in files,
                                 target.endswith("-musl"))
                self.assertTrue(all(files.values()))
        with self.assertRaisesRegex(ValueError, "require review"):
            distribution.runtime_notices("aarch64-apple-darwin", "commit-hash: unreviewed\n")
        shutil.copytree(distribution.ROOT / "licenses/runtime", self.base / "licenses/runtime")
        material = self.base / "licenses/runtime/common/libunwind-LICENSE.txt"
        material.write_bytes(b"tampered")
        with patch.object(distribution, "ROOT", self.base):
            with self.assertRaisesRegex(ValueError, "notice checksum mismatch"):
                distribution.runtime_notices("aarch64-apple-darwin", compiler)
            material.unlink()
            with self.assertRaisesRegex(ValueError, "not an ordinary file"):
                distribution.runtime_notices("aarch64-apple-darwin", compiler)

    def test_runtime_notices_survive_git_autocrlf_checkout(self):
        repository = self.base / "repository"
        repository.mkdir()
        shutil.copyfile(distribution.ROOT / ".gitattributes", repository / ".gitattributes")
        shutil.copytree(distribution.ROOT / "licenses/runtime", repository / "licenses/runtime")
        subprocess.run(["git", "init", "--quiet", str(repository)], check=True)
        git = ["git", "-C", str(repository), "-c", "core.autocrlf=true"]
        subprocess.run([*git, "add", ".gitattributes", "licenses/runtime"], check=True)
        checkout = self.base / "checkout"
        subprocess.run([*git, "checkout-index", "--all", f"--prefix={checkout.as_posix()}/"], check=True)
        source = json.loads((checkout / "licenses/runtime/common/SOURCES.json").read_text())
        compiler = f"commit-hash: {source['rust_commit']}\n"
        with patch.object(distribution, "ROOT", checkout):
            for target in distribution.TARGETS:
                with self.subTest(target=target):
                    self.assertTrue(distribution.runtime_notices(target, compiler))

    def test_release_requires_complete_eligible_exact_source(self):
        valid = self.base / "valid"
        self.release_archives(valid)
        distribution.verify_release(valid, "v0.1.0", "a" * 40)
        cases = [
            {"public_distribution_ready": False},
            {"public_distribution_ready": "true"},
            {"publication_blockers": ["runtime review"]},
            {"working_tree_dirty": True},
            {"source_commit": "b" * 40},
            {"version": "0.2.0"},
            {"publisher_signing_policy": "undecided"},
        ]
        for index, updates in enumerate(cases):
            directory = self.base / str(index)
            self.release_archives(directory, lambda provenance: provenance.update(updates))
            with self.subTest(updates=updates), self.assertRaises(ValueError):
                distribution.verify_release(directory, "v0.1.0", "a" * 40)
        for field in ("public_distribution_ready", "publication_blockers", "working_tree_dirty",
                      "publisher_signing_policy"):
            directory = self.base / field
            self.release_archives(directory, lambda provenance: provenance.pop(field))
            with self.subTest(missing=field), self.assertRaises(ValueError):
                distribution.verify_release(directory, "v0.1.0", "a" * 40)
        for tag, source in (("v0.2.0", "a" * 40), ("v0.1.0", "main")):
            with self.assertRaisesRegex(ValueError, "identity mismatch"):
                distribution.verify_release(valid, tag, source)

    def test_release_rejects_missing_duplicate_and_tampered_archives(self):
        directory = self.base / "matrix"
        self.release_archives(directory, omit="x86_64-unknown-linux-gnu")
        with self.assertRaisesRegex(ValueError, "six supported Unix targets"):
            distribution.verify_release(directory, "v0.1.0", "a" * 40)
        archive = next(directory.glob("*.tar.gz"))
        files, provenance, binary = distribution.read_archive(archive)
        provenance["archive_root"] = "duplicate"
        files["PROVENANCE.json"] = json.dumps(provenance).encode()
        self.inventory(files)
        distribution.write_archive(directory / "duplicate.tar.gz", "duplicate", files, binary)
        with self.assertRaisesRegex(ValueError, "duplicate"):
            distribution.verify_release(directory, "v0.1.0", "a" * 40)
        (directory / "duplicate.tar.gz").unlink()
        files, provenance, binary = distribution.read_archive(archive)
        files[binary] = b"tampered"
        archive.unlink()
        distribution.write_archive(archive, provenance["archive_root"], files, binary)
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            distribution.verify_release(directory, "v0.1.0", "a" * 40)
    def test_workflows_bind_source_and_gate_merge_and_publication(self):
        release = (distribution.ROOT / ".github/workflows/release.yml").read_text()
        self.assertLess(release.index("distribution.py verify-release"), release.index("gh release create"))
        self.assertNotIn("--clobber", release)
        self.assertIn("--verify-tag", release)
        self.assertIn('"--is-ancestor", sha, "origin/main"', release)
        self.assertIn("source: ${{ needs.source.outputs.sha }}", release)
        workflow = (distribution.ROOT / ".github/workflows/distribution.yml").read_text()
        self.assertEqual(set(re.findall(r"^          - target: (.+)$", workflow, re.MULTILINE)), set(distribution.TARGETS))
        self.assertIn("ref: ${{ inputs.source || github.sha }}", workflow)
        self.assertNotIn("secrets.", workflow)
        ci = (distribution.ROOT / ".github/workflows/ci.yml").read_text()
        self.assertIn("needs: [test, coverage, codecov, distribution]", ci)
        self.assertIn("needs.distribution.result == 'success'", ci)

    def test_homebrew_uses_verified_archives_and_preserves_notices(self):
        archive, _, _ = self.archive()
        recipe = homebrew.formula("0.1.0", {target: (archive, archive.as_uri()) for target in
                                  ("aarch64-apple-darwin", "x86_64-apple-darwin")})
        self.assertEqual(recipe.count(distribution.digest(archive.read_bytes())), 2)
        self.assertIn("on_intel do", recipe)
        self.assertIn('depends_on "git"', recipe)
        self.assertIn('pkgshare.install "LICENSE", "USAGE.md", "THIRD-PARTY", "THIRD-PARTY-NOTICES.md"', recipe)
        folder = self.base / "distribution"
        folder.mkdir()
        archive.rename(folder / archive.name)
        destination = self.base / "rehearsal"
        with patch.object(distribution, "native_target"), patch.object(distribution, "run") as run, \
                patch.object(distribution, "smoke"):
            homebrew.rehearse(folder, destination)
        self.assertEqual(run.call_args_list[0].args,
                         ("brew", "install-bundler-gems", "--add-groups=formula_test"))
        self.assertEqual(run.call_args_list[1].args[:2], ("brew", "ruby"))
        self.assertFalse((destination / "prefix").exists())
        self.assertIn("NEVER PUBLISH", (destination / "fluidise-relay.rb").read_text())

    def test_release_rejects_different_lockfile_or_renamed_archive(self):
        directory = self.base / "wrong-lock"
        self.release_archives(directory, lambda provenance: provenance.update(cargo_lock_sha256="0" * 64))
        with self.assertRaisesRegex(ValueError, "lockfile mismatch"):
            distribution.verify_release(directory, "v0.1.0", "a" * 40)
        directory = self.base / "renamed"
        self.release_archives(directory)
        next(directory.glob("*.tar.gz")).rename(directory / "unexpected.tar.gz")
        with self.assertRaisesRegex(ValueError, "filename/identity mismatch"):
            distribution.verify_release(directory, "v0.1.0", "a" * 40)


if __name__ == "__main__":
    unittest.main()
