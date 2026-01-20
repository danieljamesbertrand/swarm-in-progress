## Project goal (do not drift)

Build a network of interdependent nodes that:

- Communicate over an **encrypted** transport (QUIC / TLS) using **JSON** messages.
- Provide **weighted routing** (capabilities + policy) to select the best nodes for work.
- Execute **distributed AI queries** as a pipeline across many nodes/shards.
- Return the final aggregated response to the **originating requester** reliably (request-id matching, retries, timeouts).

This repo must prioritize **regression resistance** over cleverness.

## Breadcrumb rules (must be followed by every agent/dev)

- **Before changing behavior**: read `.cursor/rules/engineering_memory.md` and the most recent entries below.
- **When you change something**: append a short breadcrumb entry (date + what changed + why + how verified).
- **If a change increases risk** (protocol, routing, transport): add or update tests and keep CI green.

## Breadcrumb log

### 2026-01-20
- **Pinned toolchain + locked CI**: added `rust-toolchain.toml` and CI enforcing `cargo build/test --locked` to prevent dependency/MSRV drift regressions.
- **PowerShell guardrails**: added CI parse check over all `*.ps1` and documented quoting/safety rules in `.cursor/rules/engineering_memory.md`.
- **E2E QUIC-only integration test**: added a multi-node test proving DHT capability discovery → weighted routing → distributed AI request/response with correct `request_id` (and a deterministic “why is the sky blue?” answer), validated via `cargo test --locked`.
- **Listener response-channel regression fix**: refactored listener request handling to compute one response and call `send_response(channel, ...)` exactly once (prevents `E0382` moved-channel regressions); verified via `cargo test --locked`.
- **Test documentation**: documented what the QUIC E2E tests prove and how to run them in `docs/QUIC_E2E_TESTS.md`.
