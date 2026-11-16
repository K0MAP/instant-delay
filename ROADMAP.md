# Product and engineering roadmap

This roadmap is a working proposal based on the current codebase as of 2026-11-16. Priorities are ordered by user impact and release risk, not by implementation convenience. No milestone dates are promised; each milestone should be estimated after the preceding acceptance checks pass. See [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) for the detailed audit-driven sequence and acceptance tests.

## Current status

The Windows desktop app builds and the Rust test suite covers protocol parsing, buffering, and a synthetic ingest-to-local-sink scenario. The RTMP path is **not yet production-ready**: a complete real-encoder-to-destination run and sustained soak tests have not been established as release gates. Treat downloadable builds as experimental previews, not stable releases.

## Recommended sequence

### P0 — Make the stream path correct and measurable

**Goal:** reliably accept a real encoder stream, preserve audio/video timing, buffer it, and relay it to a test destination.

Work items:

1. Implement or adopt stateful RTMP chunk reassembly per chunk-stream ID, including extended timestamps and protocol control messages. Verify handshake, connect, createStream, publish, and teardown against a real encoder and a deterministic protocol test client.
2. Preserve audio/video/metadata ordering through the buffer. Current queue-per-media-type designs can reorder messages if drained by type; use a timestamp-ordered output queue or a merge strategy with explicit clock-wrap handling.
3. Enforce one global memory budget across video, audio, metadata, and all relay queues. Define the overflow policy (backpressure vs. controlled frame dropping) and make it visible in status.
4. Define and test delay semantics for startup, delay changes while live, zero/disabled delay, skip-to-live, reconnects, and long-running timestamp wrap.
5. Add integration tests using generated RTMP messages and a private test stream from a real encoder; test at multiple bitrates and run a soak test before calling the path stable.

**Implementation options:**

- **Maintain the in-tree RTMP implementation:** maximum control and fewer dependency assumptions, but requires ownership of protocol edge cases, test vectors, and ongoing maintenance.
- **Adopt a maintained RTMP crate:** less protocol code to maintain if it supports the required ingest/publish behavior and license/platform requirements, but adds dependency and API constraints.

**Recommendation:** first write a small compatibility spike against candidate crates and the current implementation. Choose the maintained library only if it passes the same encoder interoperability tests and handles the required RTMP/RTMPS behavior; otherwise keep the in-tree implementation behind a protocol module and finish it with comprehensive tests. Avoid adding a dependency solely to reduce line count.

**Exit criteria:** a real encoder can publish to the local server; audio/video remain synchronized; a delayed stream reaches a private destination; buffer and relay memory remain bounded during a sustained test; error states are visible and recoverable.

### P0 — Close local control and credential exposure gaps

**Goal:** keep a compromised webpage or local process from controlling the stream or reading platform credentials.

Work items:

- Add authentication and authorization for state-changing local HTTP API routes; do not rely on CORS alone as authorization.
- Redact stream keys from every status/settings/API response and logs. Keep secrets out of diagnostic exports.
- Validate RTMP destinations at connection time, including DNS resolution and private/link-local/loopback destinations; account for DNS rebinding and IPv4-mapped IPv6.
- Ensure RTMPS uses actual TLS verification; reject `rtmps://` until TLS is implemented rather than silently treating it as plain TCP.
- Add request size/time limits, rate limits, and CSRF/origin protections appropriate to the local web surface.
- Add tests proving that localhost/private destinations and unauthorized mutations are rejected.

**Recommendation:** implement credential redaction and route authorization before expanding remote-control features. Keep all local services bound to loopback by default; do not advertise LAN access until separately designed and tested.

**Exit criteria:** tests show unauthenticated requests cannot mutate configuration or retrieve secrets; hostile/ambiguous RTMP destinations are rejected; security checks cover both UI IPC and HTTP API paths.

### P1 — Relay reliability and recovery

**Goal:** make multiple output targets resilient without unbounded memory growth.

Work items:

- Model each output's state explicitly: connecting, publishing, retrying, failed, disabled.
- Add bounded queues, per-output backpressure/drop policy, byte counters, uptime, last error, and retry status.
- Implement reconnect with bounded exponential backoff and jitter; avoid duplicate publishing and stale queued media after reconnect.
- Add real connection/handshake tests with timeouts. Distinguish DNS, TCP, TLS, RTMP handshake, and publish rejection.
- Test one failed destination while other destinations continue.

**Options:** fail-fast (best for consistency, but one target can interrupt all outputs), isolate each destination (best availability, more state and memory handling), or offer a user-selected policy. **Recommendation:** isolate outputs and expose an explicit per-output state; do not let a failing destination stall healthy outputs.

### P1 — Make delay controls predictable

**Goal:** give the UI, global hotkeys, and overlay the same source of truth.

Work items:

- Specify exact behavior of Instant, Normal, and Disabled modes and align backend, overlay, and UI labels.
- Apply the configured delay step to every control path and bound all inputs against the active buffer configuration.
- Ensure changing delay while live has a documented transition policy (preserve queued content, drop/trim, or refill) and UI feedback.
- Add tests for hotkey registration, conflicts, enable/disable, and action execution; provide an accessible fallback when global shortcut permissions are unavailable.
- Make buffer, relay, and server errors visible without exposing credentials or filesystem details.

**Recommendation:** write behavior tables and state-transition tests before adding more controls or presets.

### P2 — Complete integrations and operational polish

- Finish Stream Deck command handling and add a versioned integration protocol.
- Complete supported-encoder process/version detection with safe path handling and clear unsupported-platform behavior.
- Add settings export/import with schema versioning, validation, and secret redaction by default.
- Add keyboard accessibility, localization, and onboarding that verifies the local RTMP endpoint and overlay URL.

### P2 — Repeatable releases

- Add CI for frontend build, Rust tests, formatting/lint checks, and Windows installer generation.
- Publish checksums and release notes describing tested OS/builds and known limitations.
- Sign Windows installers when a signing identity is available; without signing, state that Windows SmartScreen may warn users.
- Add dependency update/security scanning and an SBOM for release assets.
- Do not promote an experimental preview to stable until P0 exit criteria and real encoder/private-stream tests pass.

## Release gates

| Channel | Minimum bar |
| --- | --- |
| Experimental preview | Build succeeds; unit tests pass; known limitations and platform are stated; no claim of production reliability. |
| Beta | P0 stream and security criteria pass on supported Windows versions; integration and soak results are documented; recovery behavior is tested. |
| Stable | No known data/credential exposure issues; repeatable CI release; signed or clearly documented installer; regression and upgrade tests pass. |

## How to propose a roadmap change

Open an issue with the user problem, expected behavior, alternatives considered, security/privacy implications, and measurable acceptance criteria. Prefer solving correctness and reliability before adding new integrations or visual customization.
