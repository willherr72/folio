# Desktop edit latency investigation

Continue the quality goal by locating the roughly 2.27-second median end-to-end
edit delay observed in the qualified 0.39 generated 300-page workflow. The same
run used about 93 ms median browser task CPU, so browser CPU alone is not a
sufficient explanation. Accepted 0.39 and its launcher remain unchanged.

Standing user autonomy supplies the choice to investigate without approval
questions. This is diagnostic work, not a product behavior change or a new
acceptance claim. Preserve every original, sealed artifact and unsuccessful run.

## Approaches and decision

1. Time command boundaries in the existing optimized desktop, then profile the
   native engine only if needed. This preserves the actual application and avoids
   an unnecessary build. Chosen by the executor.
2. Immediately rebuild the native profiling example. Useful for native subphases,
   but does not locate UI/IPC/redraw delays by itself and costs a fresh build.
3. Change caching or validation based on existing aggregate timings. Rejected:
   the dominant delay is not yet attributed and fidelity/history must be retained.

Use the existing public generated 300-page and two-page fixtures, trusted input,
normal display, unchanged source-dependent text/raster assertions, and owned
hidden sessions. Run a 12-edit 300-page control without a network observer, a
12-edit observed 300-page run, and a 12-edit observed two-page run serially.
Record action phases, browser CPU deltas, and passive native-command start/end
timestamps. Network observation is diagnostic only: save command names, scalar
timings and request sizes, never request bodies or response text. No forced GC,
CSS injection, emulation, interception or product instrumentation.

Every edit must retain the expected alternating page text and raster; failures,
missing terminal events or incomplete observation invalidate attribution. Keep
fixture/binary/renderer/helper hashes and clean up only exact owned process/profile
identities. Do not infer normal memory bounds from instrumented runs or treat
diagnostic durations as an acceptance speed comparison. Original 105/100/100,
512 MiB/1 GiB, independent saved-file and broad quality gates remain unchanged.

If native work dominates, rebuild the existing optimized native profiling example
against the current source, preserve recorder-off controls and valid bounded
traces, and inspect the dominant phases. Only then choose a targeted implementation
with a separate concrete design/plan, meaningful regression and fidelity/performance
verification. No commit, merge, push, public release or automatic user-app launch.
