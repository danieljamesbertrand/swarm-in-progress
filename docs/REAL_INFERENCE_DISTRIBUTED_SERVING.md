# Real inference (distributed serving) – first milestone

This repo supports **distributed serving**: each node runs real local inference and the network routes requests to the best node(s) by **capability-weighted scoring**, returning a `CommandResponse` with the original `request_id`.

## Current “real inference” backend

To keep CI stable, the default is still the deterministic mock response.

The first real backend uses **`llama.cpp`** (GGUF) by invoking `llama-cli` as an external process.

## Enable real inference

Set:

- `PUNCH_INFERENCE_BACKEND=llama_cpp`
- `LLAMA_CPP_EXE` = full path to `llama-cli` / `llama-cli.exe`
- `LLAMA_GGUF_PATH` = full path to your `.gguf`

Optional:

- `LLAMA_THREADS` (defaults to CPU count)
- `LLAMA_NO_MMAP=1` (default is on; Windows often needs `--no-mmap`)

### PowerShell example

```powershell
$env:PUNCH_INFERENCE_BACKEND="llama_cpp"
$env:LLAMA_CPP_EXE="E:\rust\some-repo\llama.cpp\build\bin\Release\llama-cli.exe"
$env:LLAMA_GGUF_PATH="E:\rust\llamaModels\YourModel.gguf"
$env:LLAMA_THREADS="8"
$env:LLAMA_NO_MMAP="1"
```

## How to run a real request through the network

1) Start the listener (the executor) with any non-`mock` model name (the backend uses env vars for the GGUF path):

```powershell
cargo run --bin listener -- --transport quic --bootstrap "/ip4/127.0.0.1/udp/51820/quic-v1" --namespace simple-chat
```

2) Start the dialer and ask the question:

```powershell
cargo run --bin dialer -- --transport quic --bootstrap "/ip4/127.0.0.1/udp/51820/quic-v1" --namespace simple-chat --ask "Why is the sky blue?"
```

## Notes / limitations

- This is **distributed serving**, not pipeline-parallel shards. Each node runs a full local inference.
- `llama.cpp` invocation is per-request; for high throughput we’ll evolve toward a persistent worker process or an in-process binding.
- The pipeline-parallel coordinator/shard codepaths still contain simulation fallbacks for development. To ensure you **never** accidentally simulate, set:
  - `PUNCH_STRICT_DISTRIBUTED=1` (or `PUNCH_DISABLE_SIMULATION=1`)

