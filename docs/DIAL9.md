# dial9 runtime telemetry (autopilot)

[dial9](https://github.com/dial9-rs/dial9-tokio-telemetry) is a low-overhead
"flight recorder" for the Tokio runtime: it captures individual polls, parks,
wakes and (optionally) CPU stack samples as an event log for **post-hoc**
analysis. Unlike `tokio-console` (live debugging) or `tokio-metrics` (aggregate
counters), dial9 is meant to be left running in production and analyzed after the
fact — which is why the autopilot uploads sealed trace segments straight to S3.

Support is currently wired into the **autopilot** only.

## Building

dial9 relies on Tokio's unstable runtime hooks, so it must be built with
`--cfg tokio_unstable`. To keep normal builds (and `just clippy`, which uses
`--all-features`) unaffected, the dependency lives behind a `cfg(tokio_unstable)`
target table and a `dial9` cargo feature. Both are required:

```sh
RUSTFLAGS="--cfg tokio_unstable -C force-frame-pointers=yes" \
  cargo build --release -p autopilot --features dial9
```

- `--cfg tokio_unstable` — required; without it the `dial9` feature is a no-op
  and the crate is never compiled.
- `-C force-frame-pointers=yes` — optional but recommended; produces better CPU
  profiling stacks (the `cpu-profiling` feature is enabled).

In CI, trigger the **deploy** workflow (`workflow_dispatch`) and pick the `dial9`
predefined feature — the workflow sets the correct `RUSTFLAGS` automatically.

## Enabling at runtime

The dial9 runtime is always **disabled** unless `DIAL9_ENABLED=true`, so a
dial9-built binary behaves like a normal autopilot until switched on. Because the
runtime is constructed before the autopilot parses its own config, dial9 is
configured entirely through environment variables (`Dial9Config::from_env()`).

| Variable | Default | Meaning |
| --- | --- | --- |
| `DIAL9_ENABLED` | `false` | Master switch. Set to `true` to install telemetry. |
| `DIAL9_S3_BUCKET` | unset | Upload sealed trace segments to this bucket. If unset, traces stay on local disk only. |
| `DIAL9_SERVICE_NAME` | binary name (`autopilot`) | Service name used in S3 keys and trace metadata. |
| `DIAL9_S3_PREFIX` | `dial9-traces` | S3 object key prefix. |
| `DIAL9_TRACE_DIR` | `/tmp/dial9-traces` | Local directory for rotated segments (staging before upload). |
| `DIAL9_ROTATION_SECS` | `60` | Segment rotation period. Also the upper bound on data lost on a hard kill. |
| `DIAL9_MAX_DISK_USAGE_MB` | `1024` | Total on-disk trace budget. |
| `DIAL9_CPU_PROFILE_ENABLED` | `true` on Linux | CPU stack sampling (needs the `cpu-profiling` feature, which we build with). |
| `DIAL9_CPU_SAMPLE_HZ` | `99` | CPU sampling frequency. |

See the [crate README](https://docs.rs/dial9-tokio-telemetry) for the full list.

**S3 credentials** are loaded from the standard AWS
[`DefaultCredentialsChain`](https://docs.rs/aws-config/latest/aws_config/default_provider/credentials/struct.DefaultCredentialsChain.html)
(env vars, instance role, etc.) — the same mechanism the existing auction-instance
uploads use.

Minimal production setup for S3 archival:

```sh
DIAL9_ENABLED=true
DIAL9_S3_BUCKET=<bucket>
DIAL9_S3_PREFIX=<env>/<network>/autopilot   # e.g. staging/mainnet/autopilot
# + AWS credentials in the environment
```

## Analyzing traces

Use the `dial9` CLI (`cargo install dial9`) against the bucket or a local copy:

```sh
dial9 serve --bucket <bucket>        # browse traces directly from S3
dial9 serve --local-dir <path>       # after downloading segments
```

## Notes and caveats

- **Overhead** is typically under ~5% with Tokio + CPU profiling enabled; keep it
  disabled (`DIAL9_ENABLED` unset) by default and switch it on when investigating.
- **Whole-runtime instrumentation** (poll/park/wake timing, CPU samples) is
  captured for all tasks automatically. Richer per-task attribution (task names,
  spawn sites) requires spawning via `TelemetryHandle::spawn`; the autopilot does
  not do this yet — a possible follow-up.
- **Trace-span correlation**: dial9 ships a `tracing-layer` that can fold our
  `tracing` spans (e.g. `auction_id`) into the trace. Not wired up yet to keep the
  shared `observe` subscriber untouched — a possible follow-up.
- **Shutdown**: segments rotate and upload every `DIAL9_ROTATION_SECS` (default
  60s). On a clean exit the final segment is flushed; on `SIGKILL` up to one
  rotation period of trailing data can be lost.
