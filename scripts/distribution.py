"""Build and rehearse candidate archives; never publish or install globally."""

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "aarch64-apple-darwin": ("Darwin", "arm64"),
    "x86_64-apple-darwin": ("Darwin", "x86_64"),
    "aarch64-unknown-linux-musl": ("Linux", "aarch64"),
    "x86_64-unknown-linux-musl": ("Linux", "x86_64"),
    "aarch64-unknown-linux-gnu": ("Linux", "aarch64"),
    "x86_64-unknown-linux-gnu": ("Linux", "x86_64"),
}
MAX_ARCHIVE_BYTES = 128 * 1024 * 1024


def run(*args, cwd=ROOT, **kwargs):
    return subprocess.run(args, cwd=cwd, check=True, **kwargs)


def output(*args):
    return run(*args, capture_output=True, text=True).stdout.strip()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def regular(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"not an ordinary file: {path}")
    return path.read_bytes()


def native_target(target):
    system, machine = TARGETS[target]
    if (platform.system(), platform.machine().lower()) != (system, machine):
        raise ValueError(f"{target} requires a native {system}/{machine} runner")


def smoke(binary, version):
    """Run version outside Git, then exercise hooks in an isolated repository."""
    environment = os.environ.copy()
    for key in list(environment):
        if key.startswith("GIT_"):
            environment.pop(key)
    environment.update({"GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_SYSTEM": "/dev/null"})
    with tempfile.TemporaryDirectory(prefix="relay-smoke-") as work:
        def relay(*args, check=True):
            result = subprocess.run([str(binary), *args], cwd=work, env=environment,
                                    capture_output=True, text=True, timeout=30, check=check)
            return result

        def apply(operation):
            extra = ["--binary", str(binary)] if operation == "install" else []
            plan = json.loads(relay("inspect", "--operation", operation, *extra).stdout)["plan"]["id"]
            relay(operation, "--plan", plan, *extra)

        result = relay("--version")
        if result.stdout != f"fluidise-relay {version}\n" or result.stderr:
            raise ValueError("unexpected executable version")
        run("git", "init", "--quiet", "--template=", cwd=work, env=environment)
        repository = Path(work)
        delegate = repository / "policy"
        delegate.symlink_to(shutil.which("git"))
        (repository / "fluidise.toml").write_text(
            '[hooks]\nversion = 1\n[[hooks.commit-msg]]\nid = "policy"\n'
            'program = "./policy"\nargs = ["rev-parse", "--is-inside-work-tree"]\n'
            'modifies-message = false\n', encoding="utf-8",
        )
        message = repository / "message.txt"
        message.write_text("feat: archive test\n", encoding="utf-8")
        apply("prepare")
        apply("install")
        result = relay("run", "--file", str(message))
        if json.loads(result.stdout).get("status") != "success" or result.stderr:
            raise ValueError("unexpected hook execution result")
        # An unavailable declared delegate must fail without mutating the message.
        delegate.unlink()
        result = relay("run", "--file", str(message), check=False)
        if result.returncode != 2 or json.loads(result.stdout).get("status") != "error":
            raise ValueError("missing delegate did not fail")
        if message.read_text() != "feat: archive test\n":
            raise ValueError("smoke test changed the commit message")


def runtime_notices(target, compiler):
    """Ship the reviewed runtime texts only with their exact Rust source revision."""
    files = {}
    groups = ["common"] + (["musl-1.2.5"] if target.endswith("-musl") else [])
    for group in groups:
        directory = ROOT / "licenses/runtime" / group
        source = regular(directory / "SOURCES.json")
        inventory = json.loads(source)
        if f"commit-hash: {inventory['rust_commit']}" not in compiler.splitlines():
            raise ValueError("runtime notices require review for this Rust compiler")
        prefix = f"THIRD-PARTY/runtime/{group}"
        files[f"{prefix}/SOURCES.json"] = source
        for name, checksum in inventory["files"].items():
            data = regular(directory / name)
            if digest(data) != checksum:
                raise ValueError("runtime notice checksum mismatch")
            files[f"{prefix}/{name}"] = data
    return files


def notices(metadata, sysroot):
    files = {}
    packages = []
    resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
    for package in metadata["packages"]:
        if package["id"] not in resolved or package["id"] == metadata["resolve"]["root"]:
            continue
        base = Path(package["manifest_path"]).parent
        found = set()
        for path in base.iterdir():
            if path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "UNLICENSE")):
                if path.is_file() or path.is_symlink():
                    found.add(path)
        if package.get("license_file"):
            path = base / package["license_file"]
            if not path.resolve().is_relative_to(base.resolve()):
                raise ValueError("license file escapes package")
            found.add(path)
        if not found:
            raise ValueError(f"missing license text: {package['name']}")
        name = f"{package['name']}-{package['version']}"
        for path in sorted(found):
            relative = path.relative_to(base).as_posix()
            files[f"THIRD-PARTY/cargo/{name}/{relative}"] = regular(path)
        packages.append({
            "name": package["name"], "version": package["version"],
            "license": package.get("license"), "source": package.get("source"),
        })
    rust_docs = sysroot / "share/doc/rust"
    for name in ("COPYRIGHT.html", "COPYRIGHT-library.html"):
        files[f"THIRD-PARTY/rust/{name}"] = regular(rust_docs / name)
    licenses = rust_docs / "licenses"
    texts = sorted(licenses.glob("*.txt"))
    if not texts:
        raise ValueError("missing Rust license texts; install the rust-docs component")
    for path in texts:
        files[f"THIRD-PARTY/rust/licenses/{path.name}"] = regular(path)
    files["THIRD-PARTY-NOTICES.md"] = (
        "# Candidate third-party notices\n\n"
        "Cargo license texts and the installed Rust toolchain copyright inventory "
        "are included as a conservative superset, not a claim of exact linkage. "
        "See PROVENANCE.json for package versions and the exact compiler.\n\n"
        "Publisher signing policy for 0.1.0: no publisher signature or Apple "
        "notarization. SHA-256 checksums establish integrity relative to a trusted "
        "checksum, not publisher authenticity.\n\n"
        "Compiler-builtins, compiler-rt, libunwind and musl notices are included "
        "for the pinned compiler. System libraries remain system prerequisites; "
        "no Apple SDK or glibc binaries are redistributed.\n"
    ).encode()
    return files, packages


def write_archive(path, root_name, files, binary_name):
    with path.open("xb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", filename="", mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as archive:
            for name, data in sorted(files.items()):
                info = tarfile.TarInfo(f"{root_name}/{name}")
                info.size = len(data)
                info.mode = 0o755 if name == binary_name else 0o644
                archive.addfile(info, io.BytesIO(data))


def read_archive(path):
    """Validate the complete inventory before callers write any extracted file."""
    entries = {}
    folded = set()
    total = 0

    def add(name, size, ordinary, read):
        nonlocal total
        parts = PurePosixPath(name).parts
        if (not ordinary or "\\" in name or ":" in name or len(parts) < 2
                or any(p in ("", ".", "..") for p in parts)
                or name != "/".join(parts) or name.startswith("/")
                or name.casefold() in folded):
            raise ValueError(f"unsafe archive entry: {name}")
        total += size
        if size < 0 or total > MAX_ARCHIVE_BYTES or len(entries) >= 4096:
            raise ValueError("archive exceeds rehearsal limits")
        entries[name] = read()
        folded.add(name.casefold())

    with tarfile.open(path, "r:gz") as archive:
        for info in archive:
            add(info.name, info.size, info.isfile(), lambda info=info: archive.extractfile(info).read())
    roots = {name.split("/", 1)[0] for name in entries}
    if len(roots) != 1:
        raise ValueError("expected exactly one top-level directory")
    root_name = roots.pop()
    files = {name.split("/", 1)[1]: data for name, data in entries.items()}
    required = {"FILES.sha256", "LICENSE", "USAGE.md", "THIRD-PARTY-NOTICES.md", "PROVENANCE.json"}
    if not required.issubset(files) or not any(name.startswith("THIRD-PARTY/") for name in files):
        raise ValueError("missing required archive payload")
    inventory = files.pop("FILES.sha256")
    expected = "".join(f"{digest(data)}  {name}\n" for name, data in sorted(files.items())).encode()
    if inventory != expected:
        raise ValueError("archive inventory/checksum mismatch")
    provenance = json.loads(files["PROVENANCE.json"])
    if provenance["target"] not in TARGETS:
        raise ValueError("unsupported archive target")
    binary_name = "fluidise-relay"
    if digest(files[binary_name]) != provenance["binary_sha256"]:
        raise ValueError("binary provenance mismatch")
    if root_name != provenance["archive_root"]:
        raise ValueError("archive identity mismatch")
    files["FILES.sha256"] = inventory
    return files, provenance, binary_name


def extract_verified(path, destination):
    files, provenance, binary_name = read_archive(path)
    destination.mkdir()  # Refuse an existing destination, including symlinks.
    for name, data in files.items():
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as stream:
            stream.write(data)
        target.chmod(0o755 if name == binary_name else 0o644)
    return provenance, destination / binary_name


def prepare(target, destination):
    native_target(target)
    if destination.resolve().is_relative_to(ROOT):
        raise ValueError("candidate output must be outside the source checkout")
    destination.mkdir(parents=True, exist_ok=False)
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    version = manifest["package"]["version"]
    source = output("git", "rev-parse", "HEAD")
    dirty = bool(output("git", "status", "--porcelain", "--untracked-files=all"))
    environment = os.environ.copy()
    if target.endswith("musl"):
        environment[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = "musl-gcc"
    run("cargo", "build", "--locked", "--release", "--bin", "fluidise-relay", "--target", target, env=environment)
    metadata = json.loads(output("cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target))
    binary_name = "fluidise-relay"
    binary = Path(metadata["target_directory"]) / target / "release" / binary_name
    smoke(binary, version)
    files, packages = notices(metadata, Path(output("rustc", "--print", "sysroot")))
    compiler = output("rustc", "-vV")
    files.update(runtime_notices(target, compiler))
    files[binary_name] = regular(binary)
    for name, source_path in (("LICENSE", "LICENSE"), ("USAGE.md", "docs/usage.md")):
        files[name] = regular(ROOT / source_path)
    root_name = f"fluidise-relay-{version}-{target}-{source[:12]}" + ("-dirty" if dirty else "")
    provenance = {
        "schema": 1, "version": version, "target": target, "source_commit": source,
        "working_tree_dirty": dirty, "archive_root": root_name,
        "binary_sha256": digest(files[binary_name]),
        "cargo_lock_sha256": digest(regular(ROOT / "Cargo.lock")),
        "rustc": compiler, "cargo": output("cargo", "--version"),
        "test_environment": platform.platform(), "packages": packages,
        "publisher_signing_policy": "unsigned" if version == "0.1.0" else "undecided",
        "public_distribution_ready": version == "0.1.0" and not dirty,
        "publication_blockers": [] if (version == "0.1.0" and not dirty) else (
            (["working tree dirty"] if dirty else [])
            + ([] if version == "0.1.0" else ["publisher signing policy", "platform runtime/SDK license and redistribution review"])
        ),
    }
    files["PROVENANCE.json"] = (json.dumps(provenance, indent=2, sort_keys=True) + "\n").encode()
    files["FILES.sha256"] = "".join(f"{digest(data)}  {name}\n" for name, data in sorted(files.items())).encode()
    archive = destination / (root_name + ".tar.gz")
    write_archive(archive, root_name, files, binary_name)
    checksum = f"{digest(regular(archive))}  {archive.name}\n"
    (destination / "SHA256SUMS").write_text(checksum, encoding="utf-8")
    archive.with_name(archive.name + ".sha256").write_text(checksum, encoding="utf-8")
    with tempfile.TemporaryDirectory(prefix="relay-extract-") as work:
        _, installed = extract_verified(archive, Path(work) / "unpacked")
        smoke(installed, version)
    print(archive)


def verify_release(directory, tag, source):
    """Fail closed unless the complete matrix is eligible for this exact release."""
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    version = manifest["package"]["version"]
    if tag != f"v{version}" or not re.fullmatch(r"[0-9a-f]{40}", source):
        raise ValueError("release tag/source identity mismatch")
    targets = set()
    archives = sorted(directory.glob("*.tar.gz"))
    for archive in archives:
        regular(archive)
        _, provenance, _ = read_archive(archive)
        if archive.name != provenance["archive_root"] + ".tar.gz":
            raise ValueError("archive filename/identity mismatch")
        target = provenance["target"]
        if target in targets:
            raise ValueError("duplicate release target")
        targets.add(target)
        if (provenance.get("version") != version or provenance.get("source_commit") != source
                or provenance.get("working_tree_dirty") is not False):
            raise ValueError("release archive source/version mismatch or dirty source")
        if provenance.get("cargo_lock_sha256") != digest(regular(ROOT / "Cargo.lock")):
            raise ValueError("release lockfile mismatch")
        if (provenance.get("public_distribution_ready") is not True
                or provenance.get("publication_blockers") != []):
            raise ValueError("public distribution blocked: unresolved publication requirements")
        if version != "0.1.0" or provenance.get("publisher_signing_policy") != "unsigned":
            raise ValueError("release signing policy has not been approved")
    if targets != set(TARGETS):
        raise ValueError("release requires exactly the six supported Unix targets")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("prepare")
    build.add_argument("--target", choices=TARGETS, required=True)
    build.add_argument("--output", type=Path, required=True)
    release = commands.add_parser("verify-release")
    release.add_argument("--distribution", type=Path, required=True)
    release.add_argument("--tag", required=True)
    release.add_argument("--source", required=True)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.target, args.output.resolve())
    else:
        verify_release(args.distribution.resolve(), args.tag, args.source)


if __name__ == "__main__":
    main()
