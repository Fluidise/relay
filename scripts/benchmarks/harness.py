#!/usr/bin/env python3
"""
Performance Benchmark Harness for Fluidise Relay and Competitors.
Evaluates Native Git, Fluidise Relay, Lefthook, HK, and Husky under strictly normalized conditions.
"""

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent.parent
PAYLOADS_DIR = ROOT_DIR / "scripts" / "benchmarks" / "payloads"
RELAY_BIN = ROOT_DIR / "target" / "release" / "fluidise-relay"

TOOLS = ["native", "relay", "lefthook", "hk", "husky"]

TOOL_LABELS = {
    "native": "Native Git",
    "relay": "Fluidise Relay",
    "lefthook": "Lefthook (Go)",
    "hk": "HK (Rust)",
    "husky": "Husky (Shell)",
}


def log(msg: str):
    print(f"\033[1;34m[bench]\033[0m {msg}", flush=True)


def error(msg: str):
    print(f"\033[1;31m[bench:error]\033[0m {msg}", file=sys.stderr, flush=True)


def check_tool_available(name: str) -> bool:
    return shutil.which(name) is not None


def build_payloads(bin_dir: Path):
    """Compiles all Rust mock payloads with release optimization."""
    bin_dir.mkdir(parents=True, exist_ok=True)
    payload_files = {
        "bench-noop": PAYLOADS_DIR / "noop.rs",
        "bench-check-fast": PAYLOADS_DIR / "check_fast.rs",
        "bench-check-fmt": PAYLOADS_DIR / "check_fmt.rs",
        "bench-check-len": PAYLOADS_DIR / "check_len.rs",
        "bench-check-trailer": PAYLOADS_DIR / "check_trailer.rs",
        "bench-fail": PAYLOADS_DIR / "check_fail.rs",
    }
    for name, src in payload_files.items():
        dest = bin_dir / name
        if not dest.exists() or dest.stat().st_mtime < src.stat().st_mtime:
            log(f"Compiling payload {name}...")
            cmd = [
                "rustc",
                "-O",
                "-C", "opt-level=3",
                "-C", "panic=abort",
                "-C", "lto=fat",
                str(src),
                "-o",
                str(dest),
            ]
            subprocess.check_call(cmd)
            dest.chmod(0o755)


def verify_relay_binary():
    if not RELAY_BIN.exists():
        log("Relay release binary missing. Compiling with cargo build --release...")
        subprocess.check_call(["cargo", "build", "--release"], cwd=ROOT_DIR)
    if not RELAY_BIN.exists():
        raise RuntimeError(f"Relay binary not found at {RELAY_BIN}")


def init_git_repo(repo_dir: Path):
    if repo_dir.exists():
        shutil.rmtree(repo_dir)
    repo_dir.mkdir(parents=True, exist_ok=True)
    subprocess.check_call(["git", "init", "-q"], cwd=repo_dir)
    subprocess.check_call(["git", "config", "user.name", "Benchmark Agent"], cwd=repo_dir)
    subprocess.check_call(["git", "config", "user.email", "bench@fluidise.local"], cwd=repo_dir)


def setup_tool_repo(tool: str, repo_dir: Path, bin_dir: Path, scenario: str):
    init_git_repo(repo_dir)
    b_noop = str(bin_dir / "bench-noop")
    b_fast = str(bin_dir / "bench-check-fast")
    b_fmt = str(bin_dir / "bench-check-fmt")
    b_len = str(bin_dir / "bench-check-len")
    b_trailer = str(bin_dir / "bench-check-trailer")
    b_fail = str(bin_dir / "bench-fail")

    if tool == "native":
        hook_path = repo_dir / ".git" / "hooks" / "commit-msg"
        hook_path.parent.mkdir(parents=True, exist_ok=True)
        if scenario == "S0":
            content = f"#!/bin/sh\nexec '{b_noop}'\n"
        elif scenario == "S1":
            content = f"#!/bin/sh\nexec '{b_fast}' \"$1\"\n"
        elif scenario == "S2":
            content = (
                f"#!/bin/sh\n'{b_fmt}' \"$1\" || exit 1\n"
                f"'{b_len}' \"$1\" || exit 1\n"
                f"exec '{b_trailer}' \"$1\"\n"
            )
        elif scenario == "S3":
            content = f"#!/bin/sh\n'{b_fast}' \"$1\" || exit 1\nexec '{b_fail}'\n"
        else:
            raise ValueError(f"Unknown scenario {scenario}")
        hook_path.write_text(content)
        hook_path.chmod(0o755)

    elif tool == "relay":
        cfg_path = repo_dir / "fluidise.toml"
        if scenario == "S0":
            cfg = f"""[hooks]
version = 1
timeout-seconds = 60

[[hooks.commit-msg]]
id = "step1"
program = "{b_noop}"
args = []
modifies-message = false
"""
        elif scenario == "S1":
            cfg = f"""[hooks]
version = 1
timeout-seconds = 60

[[hooks.commit-msg]]
id = "step1"
program = "{b_fast}"
args = ["{{message-file}}"]
modifies-message = false
"""
        elif scenario == "S2":
            cfg = f"""[hooks]
version = 1
timeout-seconds = 60

[[hooks.commit-msg]]
id = "step1"
program = "{b_fmt}"
args = ["{{message-file}}"]
modifies-message = false

[[hooks.commit-msg]]
id = "step2"
program = "{b_len}"
args = ["{{message-file}}"]
modifies-message = false

[[hooks.commit-msg]]
id = "step3"
program = "{b_trailer}"
args = ["{{message-file}}"]
modifies-message = false
"""
        elif scenario == "S3":
            cfg = f"""[hooks]
version = 1
timeout-seconds = 60

[[hooks.commit-msg]]
id = "step1"
program = "{b_fast}"
args = ["{{message-file}}"]
modifies-message = false

[[hooks.commit-msg]]
id = "step2"
program = "{b_fail}"
args = []
modifies-message = false
"""
        else:
            raise ValueError(f"Unknown scenario {scenario}")
        cfg_path.write_text(cfg)

        # Inspect and prepare
        out_prep = subprocess.check_output(
            [str(RELAY_BIN), "inspect", "--operation", "prepare", "--repo", str(repo_dir)]
        )
        plan_prep = json.loads(out_prep)["plan"]["id"]
        subprocess.check_call(
            [str(RELAY_BIN), "prepare", "--repo", str(repo_dir), "--plan", plan_prep],
            stdout=subprocess.DEVNULL,
        )

        # Inspect and install
        out_inst = subprocess.check_output(
            [
                str(RELAY_BIN),
                "inspect",
                "--operation",
                "install",
                "--repo",
                str(repo_dir),
                "--binary",
                str(RELAY_BIN),
            ]
        )
        plan_inst = json.loads(out_inst)["plan"]["id"]
        subprocess.check_call(
            [
                str(RELAY_BIN),
                "install",
                "--repo",
                str(repo_dir),
                "--binary",
                str(RELAY_BIN),
                "--plan",
                plan_inst,
            ],
            stdout=subprocess.DEVNULL,
        )

    elif tool == "lefthook":
        cfg_path = repo_dir / "lefthook.yml"
        if scenario == "S0":
            cfg = f"""commit-msg:
  commands:
    step1:
      run: '{b_noop}'
"""
        elif scenario == "S1":
            cfg = f"""commit-msg:
  commands:
    step1:
      run: '{b_fast} {{1}}'
"""
        elif scenario == "S2":
            cfg = f"""commit-msg:
  commands:
    step1:
      run: '{b_fmt} {{1}}'
    step2:
      run: '{b_len} {{1}}'
    step3:
      run: '{b_trailer} {{1}}'
"""
        elif scenario == "S3":
            cfg = f"""commit-msg:
  commands:
    step1:
      run: '{b_fast} {{1}}'
    step2:
      run: '{b_fail}'
"""
        else:
            raise ValueError(f"Unknown scenario {scenario}")
        cfg_path.write_text(cfg)
        subprocess.check_call(["lefthook", "install"], cwd=repo_dir, stdout=subprocess.DEVNULL)

    elif tool == "hk":
        cfg_path = repo_dir / "hk.pkl"
        if scenario == "S0":
            cfg = f"""amends "package://github.com/jdx/hk/releases/download/v2.7.1/hk@2.7.1#/Config.pkl"

hooks {{
  ["commit-msg"] {{
    steps {{
      ["step1"] {{
        check = "{b_noop}"
      }}
    }}
  }}
}}
"""
        elif scenario == "S1":
            cfg = f"""amends "package://github.com/jdx/hk/releases/download/v2.7.1/hk@2.7.1#/Config.pkl"

hooks {{
  ["commit-msg"] {{
    steps {{
      ["step1"] {{
        check = "{b_fast} {{{{commit_msg_file}}}}"
      }}
    }}
  }}
}}
"""
        elif scenario == "S2":
            cfg = f"""amends "package://github.com/jdx/hk/releases/download/v2.7.1/hk@2.7.1#/Config.pkl"

hooks {{
  ["commit-msg"] {{
    steps {{
      ["step1"] {{
        check = "{b_fmt} {{{{commit_msg_file}}}}"
      }}
      ["step2"] {{
        check = "{b_len} {{{{commit_msg_file}}}}"
      }}
      ["step3"] {{
        check = "{b_trailer} {{{{commit_msg_file}}}}"
      }}
    }}
  }}
}}
"""
        elif scenario == "S3":
            cfg = f"""amends "package://github.com/jdx/hk/releases/download/v2.7.1/hk@2.7.1#/Config.pkl"

hooks {{
  ["commit-msg"] {{
    steps {{
      ["step1"] {{
        check = "{b_fast} {{{{commit_msg_file}}}}"
      }}
      ["step2"] {{
        check = "{b_fail}"
      }}
    }}
  }}
}}
"""
        else:
            raise ValueError(f"Unknown scenario {scenario}")
        cfg_path.write_text(cfg)
        subprocess.check_call(["hk", "install"], cwd=repo_dir, stdout=subprocess.DEVNULL)

    elif tool == "husky":
        subprocess.check_call(["husky"], cwd=repo_dir, stdout=subprocess.DEVNULL)
        hook_path = repo_dir / ".husky" / "commit-msg"
        hook_path.parent.mkdir(parents=True, exist_ok=True)
        if scenario == "S0":
            content = f"#!/usr/bin/env sh\nexec '{b_noop}'\n"
        elif scenario == "S1":
            content = f"#!/usr/bin/env sh\nexec '{b_fast}' \"$1\"\n"
        elif scenario == "S2":
            content = (
                f"#!/usr/bin/env sh\n'{b_fmt}' \"$1\" || exit 1\n"
                f"'{b_len}' \"$1\" || exit 1\n"
                f"exec '{b_trailer}' \"$1\"\n"
            )
        elif scenario == "S3":
            content = f"#!/usr/bin/env sh\n'{b_fast}' \"$1\" || exit 1\nexec '{b_fail}'\n"
        else:
            raise ValueError(f"Unknown scenario {scenario}")
        hook_path.write_text(content)
        hook_path.chmod(0o755)


def get_hook_entrypoint(tool: str, repo_dir: Path) -> Path:
    if tool == "native":
        return repo_dir / ".git" / "hooks" / "commit-msg"
    elif tool == "relay":
        return repo_dir / ".git" / "fluidise-relay" / "hooks" / "commit-msg"
    elif tool == "lefthook":
        return repo_dir / ".git" / "hooks" / "commit-msg"
    elif tool == "hk":
        return repo_dir / ".git" / "hooks" / "commit-msg"
    elif tool == "husky":
        return repo_dir / ".husky" / "_" / "commit-msg"
    else:
        raise ValueError(f"Unknown tool {tool}")


def measure_peak_rss_and_processes(cmd: list[str], cwd: Path) -> tuple[int, int]:
    """Measures peak resident set size in KiB and estimated process count."""
    is_mac = platform.system() == "Darwin"
    if is_mac:
        time_cmd = ["/usr/bin/time", "-l"] + cmd
    else:
        time_cmd = ["/usr/bin/time", "-v"] + cmd

    try:
        proc = subprocess.run(
            time_cmd,
            cwd=cwd,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        stderr = proc.stderr
        peak_rss_kb = 0
        if is_mac:
            m = re.search(r"(\d+)\s+maximum resident set size", stderr)
            if m:
                # macOS time -l reports bytes
                peak_rss_kb = int(m.group(1)) // 1024
        else:
            m = re.search(r"Maximum resident set size \(kbytes\):\s+(\d+)", stderr)
            if m:
                peak_rss_kb = int(m.group(1))

        # Estimate process count: run through trace or baseline estimation
        proc_count = 1
        return peak_rss_kb, proc_count
    except Exception as e:
        error(f"Error measuring memory: {e}")
        return 0, 1


def run_tier1_scenario(
    scenario: str,
    repos: dict[str, Path],
    msg_file: Path,
    warmup: int,
    min_runs: int,
    out_dir: Path,
) -> dict:
    log(f"=== Running Tier 1 (Micro-Orchestrator) for {scenario} ===")
    json_path = out_dir / f"tier1_{scenario}.json"

    hyperfine_cmd = [
        "hyperfine",
        "--shell=none",
        f"--warmup={warmup}",
        f"--min-runs={min_runs}",
        "--export-json",
        str(json_path),
    ]

    if scenario == "S3":
        hyperfine_cmd.append("--ignore-failure=1,2")

    for tool in TOOLS:
        hook_bin = get_hook_entrypoint(tool, repos[tool])
        label = TOOL_LABELS[tool]
        hyperfine_cmd.extend(["-n", label, f"{hook_bin} {msg_file}"])

    subprocess.check_call(hyperfine_cmd)

    with open(json_path) as f:
        data = json.load(f)

    # Measure memory for each tool
    memory_stats = {}
    for tool in TOOLS:
        hook_bin = get_hook_entrypoint(tool, repos[tool])
        rss_kb, _ = measure_peak_rss_and_processes([str(hook_bin), str(msg_file)], repos[tool])
        memory_stats[TOOL_LABELS[tool]] = rss_kb

    return {"hyperfine": data, "memory_kb": memory_stats}


def run_tier2_scenario(
    scenario: str,
    repos: dict[str, Path],
    msg_file: Path,
    warmup: int,
    min_runs: int,
    out_dir: Path,
) -> dict:
    log(f"=== Running Tier 2 (Macro Git Commit) for {scenario} ===")
    json_path = out_dir / f"tier2_{scenario}.json"

    hyperfine_cmd = [
        "hyperfine",
        f"--warmup={warmup}",
        f"--min-runs={min_runs}",
        "--export-json",
        str(json_path),
    ]

    if scenario == "S3":
        hyperfine_cmd.append("--ignore-failure=1,2")

    for tool in TOOLS:
        repo_dir = repos[tool]
        label = TOOL_LABELS[tool]
        cmd_str = f"git -C '{repo_dir}' commit --allow-empty -F '{msg_file}'"
        hyperfine_cmd.extend(["-n", label, cmd_str])

    subprocess.check_call(hyperfine_cmd)

    with open(json_path) as f:
        data = json.load(f)

    return {"hyperfine": data}


def extract_metric(r: dict, metric_name: str, key: str = "median") -> float:
    """Extracts a metric from either Hyperfine 2.x summary or 1.x flat format."""
    summary = r.get("summary", {})
    if metric_name in summary and key in summary[metric_name]:
        return float(summary[metric_name][key])
    elif metric_name in r:
        return float(r[metric_name])
    return 0.0


def extract_wall_clock(r: dict) -> dict[str, float]:
    """Returns a dictionary with mean, median, min, max, stddev in seconds."""
    summary = r.get("summary", {})
    if "time_wall_clock" in summary:
        wc = summary["time_wall_clock"]
        return {
            "mean": float(wc.get("mean", 0.0)),
            "median": float(wc.get("median", 0.0)),
            "min": float(wc.get("min", 0.0)),
            "max": float(wc.get("max", 0.0)),
            "stddev": float(wc.get("stddev", 0.0)),
        }
    return {
        "mean": float(r.get("mean", 0.0)),
        "median": float(r.get("median", 0.0)),
        "min": float(r.get("min", 0.0)),
        "max": float(r.get("max", 0.0)),
        "stddev": float(r.get("stddev", 0.0)),
    }


def generate_markdown_report(all_results: dict, out_path: Path):
    lines = [
        "# Fluidise Relay — Benchmark Comparative Results",
        "",
        f"**Date**: {time.strftime('%Y-%m-%d %H:%M:%S')}",
        f"**Platform**: {platform.system()} {platform.machine()} ({platform.release()})",
        "",
        "---",
        "",
    ]

    for scenario, sdata in all_results.items():
        lines.append(f"## Scenario {scenario}")
        lines.append("")

        if "tier1" in sdata:
            lines.append("### Tier 1: Micro-Orchestrator Overhead (Direct Hook Execution)")
            lines.append("")
            lines.append(
                "| Tool | Median (ms) | Mean ± StdDev (ms) | Min … Max (ms) | Net Overhead Δt (ms) | Peak RSS (KiB) | CPU User / Sys (ms) |"
            )
            lines.append(
                "| :--- | :--- | :--- | :--- | :--- | :--- | :--- |"
            )

            hf = sdata["tier1"]["hyperfine"]["results"]
            mem_external = sdata["tier1"].get("memory_kb", {})

            # Find native baseline median
            native_median = 0.0
            for r in hf:
                name = r.get("name") or r.get("command")
                if name == "Native Git":
                    native_median = extract_wall_clock(r)["median"] * 1000.0
                    break

            for r in hf:
                name = r.get("name") or r.get("command")
                wc = extract_wall_clock(r)
                mean_ms = wc["mean"] * 1000.0
                median_ms = wc["median"] * 1000.0
                min_ms = wc["min"] * 1000.0
                max_ms = wc["max"] * 1000.0
                stddev_ms = wc["stddev"] * 1000.0

                delta_ms = median_ms - native_median if name != "Native Git" else 0.0
                delta_str = f"+{delta_ms:.2f} ms" if delta_ms > 0 else f"{delta_ms:.2f} ms (ref)"

                # Peak RSS from hyperfine 2.x or fallback to time -l
                peak_bytes = extract_metric(r, "memory_peak_resident", "median")
                if peak_bytes > 0:
                    rss_kb = int(peak_bytes // 1024)
                else:
                    rss_kb = mem_external.get(name, 0)
                rss_str = f"{rss_kb:,} KiB" if rss_kb > 0 else "N/A"

                user_ms = extract_metric(r, "time_user", "median") * 1000.0
                sys_ms = extract_metric(r, "time_system", "median") * 1000.0
                cpu_str = f"{user_ms:.1f} / {sys_ms:.1f} ms"

                lines.append(
                    f"| **{name}** | {median_ms:.2f} | {mean_ms:.2f} ± {stddev_ms:.2f} | {min_ms:.2f} … {max_ms:.2f} | {delta_str} | {rss_str} | {cpu_str} |"
                )
            lines.append("")

        if "tier2" in sdata:
            lines.append("### Tier 2: Macro Git Commit Latency (`git commit`)")
            lines.append("")
            lines.append(
                "| Tool | Median (ms) | Mean ± StdDev (ms) | Min … Max (ms) | Net Overhead Δt (ms) |"
            )
            lines.append(
                "| :--- | :--- | :--- | :--- | :--- |"
            )

            hf = sdata["tier2"]["hyperfine"]["results"]
            native_median = 0.0
            for r in hf:
                name = r.get("name") or r.get("command")
                if name == "Native Git":
                    native_median = extract_wall_clock(r)["median"] * 1000.0
                    break

            for r in hf:
                name = r.get("name") or r.get("command")
                wc = extract_wall_clock(r)
                mean_ms = wc["mean"] * 1000.0
                median_ms = wc["median"] * 1000.0
                min_ms = wc["min"] * 1000.0
                max_ms = wc["max"] * 1000.0
                stddev_ms = wc["stddev"] * 1000.0

                delta_ms = median_ms - native_median if name != "Native Git" else 0.0
                delta_str = f"+{delta_ms:.2f} ms" if delta_ms > 0 else f"{delta_ms:.2f} ms (ref)"

                lines.append(
                    f"| **{name}** | {median_ms:.2f} | {mean_ms:.2f} ± {stddev_ms:.2f} | {min_ms:.2f} … {max_ms:.2f} | {delta_str} |"
                )
            lines.append("")

    report_content = "\n".join(lines) + "\n"
    out_path.write_text(report_content)
    log(f"Markdown report generated at {out_path}")
    print("\n" + report_content)


def main():
    parser = argparse.ArgumentParser(description="Relay Competitors Benchmark Harness")
    parser.add_argument(
        "--scenario",
        choices=["S0", "S1", "S2", "S3", "all"],
        default="S1",
        help="Benchmark scenario to run (default: S1)",
    )
    parser.add_argument(
        "--tier",
        choices=["1", "2", "all"],
        default="1",
        help="Benchmark tier to execute (1: Micro-hook, 2: Macro-git, all: both)",
    )
    parser.add_argument("--warmup", type=int, default=15, help="Warmup iterations (default: 15)")
    parser.add_argument("--runs", type=int, default=100, help="Benchmark runs (default: 100)")
    parser.add_argument(
        "--work-dir",
        type=Path,
        default=None,
        help="Working directory for test repos (default: auto tempdir)",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=ROOT_DIR / "docs" / "benchmarks",
        help="Output directory for reports",
    )
    args = parser.parse_args()

    # Preconditions check
    verify_relay_binary()
    for tool_cmd in ["hyperfine", "lefthook", "hk", "husky", "git", "rustc"]:
        if not check_tool_available(tool_cmd):
            error(f"{tool_cmd} is required but not found on PATH.")
            sys.exit(1)

    work_dir = args.work_dir
    cleanup_temp = False
    if work_dir is None:
        temp_dir = tempfile.mkdtemp(prefix="relay-bench-")
        work_dir = Path(temp_dir)
        cleanup_temp = True

    try:
        log(f"Working directory: {work_dir}")
        bin_dir = work_dir / "bin"
        build_payloads(bin_dir)

        # Standard commit message
        msg_file = work_dir / "COMMIT_MSG"
        msg_file.write_text(
            "feat(bench): implement automated competitor benchmark\n\n"
            "This commit message tests real parsing across all hook managers.\n"
        )

        scenarios = ["S0", "S1", "S2", "S3"] if args.scenario == "all" else [args.scenario]
        run_tier1 = args.tier in ["1", "all"]
        run_tier2 = args.tier in ["2", "all"]

        all_results = {}
        args.output_dir.mkdir(parents=True, exist_ok=True)

        for sc in scenarios:
            log(f"--- Preparing fixtures for Scenario {sc} ---")
            repos = {}
            for tool in TOOLS:
                r_dir = work_dir / f"repo-{tool}-{sc}"
                setup_tool_repo(tool, r_dir, bin_dir, sc)
                repos[tool] = r_dir

            all_results[sc] = {}

            if run_tier1:
                t1_res = run_tier1_scenario(
                    sc, repos, msg_file, args.warmup, args.runs, args.output_dir
                )
                all_results[sc]["tier1"] = t1_res

            if run_tier2:
                t2_res = run_tier2_scenario(
                    sc, repos, msg_file, args.warmup, args.runs, args.output_dir
                )
                all_results[sc]["tier2"] = t2_res

        # Save aggregated JSON
        json_summary_path = args.output_dir / "benchmark-summary.json"
        with open(json_summary_path, "w") as f:
            json.dump(all_results, f, indent=2)
        log(f"Aggregated JSON results saved to {json_summary_path}")

        # Render Markdown report
        md_report_path = args.output_dir / "benchmark-results.md"
        generate_markdown_report(all_results, md_report_path)

    finally:
        if cleanup_temp:
            log(f"Cleaning up temporary directory {work_dir}...")
            shutil.rmtree(work_dir, ignore_errors=True)


if __name__ == "__main__":
    main()
