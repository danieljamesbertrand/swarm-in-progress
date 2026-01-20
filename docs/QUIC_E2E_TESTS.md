# QUIC End-to-End (E2E) Integration Tests

This repo includes **deterministic QUIC-only** integration tests that prove the core promise of the system:

- **Discovery** (without relying on flaky DHT propagation timing)
- **Weighted routing**
- **Distributed execution**
- **Response returns to the original requester** with the correct `request_id`
- A **real (guardrailed) answer** for: “Why is the sky blue?”

These are intended to be safe for CI: they run locally on `127.0.0.1`, use ephemeral ports (`udp/0`), and complete quickly.

## Tests and what they prove

### 1) 3-node topology: rendezvous server → listener → dialer

- **Test file**: `tests/e2e_quic_server_listener_dialer_tests.rs`
- **Test name**: `test_e2e_quic_server_listener_dialer_question`

**Topology**

- **server**: rendezvous/registry (central lookup)
- **listener**: registers its QUIC listen addrs and answers AI questions
- **dialer**: asks server for listener record, dials listener directly, sends AI question

**What is validated**

- **Discovery**: listener **registers** a `PeerDiscoveryRecord` (peer id + QUIC listen addrs) to server; dialer performs **lookup** by namespace.
- **Direct QUIC dial**: dialer extracts a `quic-v1` address from the lookup results and dials the listener.
- **Distributed execution contract**:
  - dialer sends a JSON `Command` with `command=EXECUTE_TASK` and `task_type=ai_inference`
  - listener replies with a JSON `CommandResponse`
- **Correct correlation**:
  - dialer asserts `CommandResponse.request_id == Command.request_id`
- **“Real answer” guardrails** (must be present in response output):
  - mentions `rayleigh`
  - mentions `scatter`
  - mentions `wavelength`

### 2) Multi-node: discovery → weighted routing → distributed execution → aggregated answer

- **Test file**: `tests/e2e_quic_weighted_routing_ai_tests.rs`
- **Test name**: `test_e2e_quic_discovery_weighted_routing_distributed_ai`

**Topology**

- **coordinator**: receives worker registrations and dispatches tasks
- **worker_fast**: high-capability worker
- **worker_slow**: lower-capability worker

**What is validated**

- **Discovery/registration**: both workers send `DiscoveryMessage::Register` with `NodeCapabilities` over QUIC request/response.
- **Weighted routing prefers fast worker**:
  - calculates scores (`NodeCapabilities::calculate_score(NodeWeights)`)
  - asserts `fast_score > slow_score`
- **Distributed execution with shared `request_id`**:
  - coordinator sends the same `request_id` to both workers
  - asserts every `CommandResponse` echoes back the same `request_id`
- **Aggregated “real answer” guardrails** across the combined outputs:
  - includes `rayleigh`
  - includes `scatter`
  - includes `shorter` (wavelengths)
  - includes a sunrise/sunset color-shift mention (`sunset`/`sunrise`/`reds`/`oranges`)

## How to run

Run the 3-node test:

```bash
cargo test --locked --test e2e_quic_server_listener_dialer_tests -- --nocapture
```

Run the 3-node test with a **full step-by-step trace** (recommended when validating an end-to-end AI request visually):

```bash
PUNCH_TRACE=1 cargo test --locked --test e2e_quic_server_listener_dialer_tests -- --nocapture
```

Run the weighted-routing test:

```bash
cargo test --locked --test e2e_quic_weighted_routing_ai_tests -- --nocapture
```

Run both:

```bash
cargo test --locked --test e2e_quic_server_listener_dialer_tests -- --nocapture
cargo test --locked --test e2e_quic_weighted_routing_ai_tests -- --nocapture
```

## Notes

- These tests are **QUIC-only** (`/udp/.../quic-v1`) to enforce the project’s “QUIC first” direction.
- They intentionally avoid relying on DHT record propagation timing for discovery; they use explicit request/response registration handshakes to be deterministic.

