# Fluidise Relay — Benchmark Comparative Results

**Date**: 2026-10-25 16:46:45
**Platform**: Darwin arm64 (25.6.0)

---

## Scenario S0

### Tier 1: Micro-Orchestrator Overhead (Direct Hook Execution)

| Tool | Median (ms) | Mean ± StdDev (ms) | Min … Max (ms) | Net Overhead Δt (ms) | Peak RSS (KiB) | CPU User / Sys (ms) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Native Git** | 5.90 | 6.19 ± 1.63 | 5.05 … 34.09 | 0.00 ms (ref) | 1,424 KiB | 2.1 / 2.1 ms |
| **Fluidise Relay** | 9.45 | 9.76 ± 1.88 | 7.72 … 27.10 | +3.55 ms | 2,064 KiB | 3.5 / 3.5 ms |
| **Lefthook (Go)** | 52.38 | 52.44 ± 2.91 | 48.29 … 63.35 | +46.49 ms | 15,248 KiB | 24.5 / 21.6 ms |
| **HK (Rust)** | 11.50 | 11.48 ± 0.64 | 10.25 … 13.78 | +5.60 ms | 12,704 KiB | 5.2 / 4.9 ms |
| **Husky (Shell)** | 22.63 | 22.57 ± 1.65 | 19.31 … 26.66 | +16.73 ms | 2,160 KiB | 7.2 / 10.2 ms |

### Tier 2: Macro Git Commit Latency (`git commit`)

| Tool | Median (ms) | Mean ± StdDev (ms) | Min … Max (ms) | Net Overhead Δt (ms) |
| :--- | :--- | :--- | :--- | :--- |
| **Native Git** | 32.52 | 32.76 ± 3.01 | 27.55 … 43.41 | 0.00 ms (ref) |
| **Fluidise Relay** | 34.81 | 35.29 ± 3.30 | 30.07 … 47.62 | +2.29 ms |
| **Lefthook (Go)** | 85.69 | 86.82 ± 6.56 | 76.52 … 99.42 | +53.17 ms |
| **HK (Rust)** | 58.54 | 58.68 ± 4.76 | 52.23 … 67.45 | +26.03 ms |
| **Husky (Shell)** | 110.34 | 110.94 ± 11.06 | 88.46 … 140.13 | +77.82 ms |

## Scenario S1

### Tier 1: Micro-Orchestrator Overhead (Direct Hook Execution)

| Tool | Median (ms) | Mean ± StdDev (ms) | Min … Max (ms) | Net Overhead Δt (ms) | Peak RSS (KiB) | CPU User / Sys (ms) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Native Git** | 5.40 | 5.53 ± 0.48 | 4.91 … 7.99 | 0.00 ms (ref) | 1,440 KiB | 2.0 / 1.9 ms |
| **Fluidise Relay** | 8.43 | 8.85 ± 1.30 | 7.62 … 15.81 | +3.03 ms | 2,064 KiB | 3.3 / 3.2 ms |
| **Lefthook (Go)** | 50.85 | 51.48 ± 3.32 | 48.25 … 63.51 | +45.45 ms | 15,296 KiB | 24.1 / 20.9 ms |
| **HK (Rust)** | 11.15 | 11.45 ± 0.94 | 10.21 … 14.84 | +5.75 ms | 12,688 KiB | 5.0 / 4.9 ms |
| **Husky (Shell)** | 27.60 | 26.88 ± 3.48 | 20.04 … 39.67 | +22.19 ms | 2,176 KiB | 8.1 / 13.1 ms |

### Tier 2: Macro Git Commit Latency (`git commit`)

| Tool | Median (ms) | Mean ± StdDev (ms) | Min … Max (ms) | Net Overhead Δt (ms) |
| :--- | :--- | :--- | :--- | :--- |
| **Native Git** | 31.95 | 32.45 ± 5.76 | 26.84 … 76.94 | 0.00 ms (ref) |
| **Fluidise Relay** | 31.45 | 32.57 ± 2.62 | 29.62 … 39.62 | -0.50 ms (ref) |
| **Lefthook (Go)** | 79.22 | 81.51 ± 4.64 | 75.97 … 89.11 | +47.27 ms |
| **HK (Rust)** | 58.98 | 59.13 ± 5.06 | 51.14 … 68.53 | +27.03 ms |
| **Husky (Shell)** | 103.02 | 108.40 ± 23.91 | 87.71 … 211.88 | +71.07 ms |

## Scenario S2

### Tier 1: Micro-Orchestrator Overhead (Direct Hook Execution)

| Tool | Median (ms) | Mean ± StdDev (ms) | Min … Max (ms) | Net Overhead Δt (ms) | Peak RSS (KiB) | CPU User / Sys (ms) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Native Git** | 10.29 | 10.67 ± 0.85 | 9.65 … 13.29 | 0.00 ms (ref) | 1,440 KiB | 2.1 / 2.3 ms |
| **Fluidise Relay** | 12.93 | 13.77 ± 2.44 | 11.66 … 23.83 | +2.64 ms | 2,272 KiB | 5.1 / 4.9 ms |
| **Lefthook (Go)** | 50.86 | 51.82 ± 2.93 | 48.14 … 57.92 | +40.57 ms | 15,232 KiB | 24.2 / 21.4 ms |
| **HK (Rust)** | 11.38 | 11.36 ± 0.61 | 10.30 … 14.27 | +1.09 ms | 12,688 KiB | 5.0 / 5.0 ms |
| **Husky (Shell)** | 25.42 | 26.09 ± 1.42 | 24.20 … 29.76 | +15.13 ms | 2,160 KiB | 6.9 / 9.8 ms |

### Tier 2: Macro Git Commit Latency (`git commit`)

| Tool | Median (ms) | Mean ± StdDev (ms) | Min … Max (ms) | Net Overhead Δt (ms) |
| :--- | :--- | :--- | :--- | :--- |
| **Native Git** | 32.87 | 33.86 ± 2.04 | 31.59 … 43.26 | 0.00 ms (ref) |
| **Fluidise Relay** | 36.46 | 37.33 ± 3.45 | 33.87 … 46.01 | +3.58 ms |
| **Lefthook (Go)** | 93.47 | 95.03 ± 5.90 | 88.18 … 104.61 | +60.60 ms |
| **HK (Rust)** | 59.08 | 60.03 ± 3.45 | 55.43 … 67.99 | +26.21 ms |
| **Husky (Shell)** | 101.33 | 100.77 ± 7.46 | 91.90 … 115.44 | +68.46 ms |

## Scenario S3

### Tier 1: Micro-Orchestrator Overhead (Direct Hook Execution)

| Tool | Median (ms) | Mean ± StdDev (ms) | Min … Max (ms) | Net Overhead Δt (ms) | Peak RSS (KiB) | CPU User / Sys (ms) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Native Git** | 7.76 | 8.14 ± 0.85 | 7.38 … 14.41 | 0.00 ms (ref) | 1,440 KiB | 2.1 / 2.1 ms |
| **Fluidise Relay** | 10.57 | 10.54 ± 0.62 | 9.66 … 13.01 | +2.81 ms | 2,208 KiB | 4.2 / 4.0 ms |
| **Lefthook (Go)** | 50.98 | 52.15 ± 2.95 | 48.58 … 58.34 | +43.21 ms | 15,280 KiB | 24.2 / 21.7 ms |
| **HK (Rust)** | 11.28 | 11.35 ± 0.60 | 10.38 … 12.89 | +3.52 ms | 12,688 KiB | 5.0 / 5.0 ms |
| **Husky (Shell)** | 22.94 | 23.87 ± 1.70 | 22.15 … 28.34 | +15.17 ms | 2,160 KiB | 6.8 / 9.6 ms |

### Tier 2: Macro Git Commit Latency (`git commit`)

| Tool | Median (ms) | Mean ± StdDev (ms) | Min … Max (ms) | Net Overhead Δt (ms) |
| :--- | :--- | :--- | :--- | :--- |
| **Native Git** | 15.67 | 16.12 ± 1.13 | 14.74 … 20.86 | 0.00 ms (ref) |
| **Fluidise Relay** | 20.03 | 21.31 ± 3.45 | 17.34 … 37.19 | +4.37 ms |
| **Lefthook (Go)** | 69.55 | 71.08 ± 3.48 | 67.17 … 78.70 | +53.88 ms |
| **HK (Rust)** | 39.68 | 41.13 ± 2.42 | 38.51 … 45.87 | +24.02 ms |
| **Husky (Shell)** | 61.42 | 62.88 ± 3.11 | 59.06 … 71.50 | +45.75 ms |
