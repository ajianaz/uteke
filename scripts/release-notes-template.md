### 📦 Downloads

Each archive contains three binaries + ONNX Runtime shared library:

| Binary | Description |
|--------|-------------|
| `uteke` | CLI tool |
| `uteke-serve` | HTTP server daemon |
| `uteke-mcp` | MCP server (stdio + HTTP) |
| `libonnxruntime.so*` / `libonnxruntime*.dylib` / `onnxruntime*.dll` | ONNX Runtime |

| Platform | File |
|----------|------|
| Linux x86_64 (AVX2) | `uteke-x86_64-unknown-linux-gnu-v__VER__.tar.gz` |
| Linux x86_64 (Legacy, SSE4.2) | `uteke-x86_64-unknown-linux-gnu-legacy-v__VER__.tar.gz` |
| Linux ARM64 | `uteke-aarch64-unknown-linux-gnu-v__VER__.tar.gz` |
| macOS Apple Silicon | `uteke-aarch64-apple-darwin-v__VER__.tar.gz` |
| Windows x86_64 | `uteke-x86_64-pc-windows-msvc-v__VER__.zip` |

> **Legacy Bundle (Linux only)** — Includes a SSE4.2-only ORT sidecar (`ort-legacy/`) for CPUs without AVX2 (Intel Celeron J4125/N4020). Use this bundle to avoid SIGILL crashes on older hardware.

### 🚀 Quick Start

```bash
# Quick install (Linux / macOS)
curl -fsSL https://raw.githubusercontent.com/codecoradev/uteke/main/install.sh | sh

# Pin a specific version
UTEKE_VERSION=__VER__ curl -fsSL https://raw.githubusercontent.com/codecoradev/uteke/main/install.sh | sh

# Store a memory
uteke remember "Important context" --tags project

# Recall by meaning
uteke recall "what was that context?"

# Start server for fast AI agent access
uteke-serve --port 8767
```

**Full changelog:** https://github.com/codecoradev/uteke/blob/main/CHANGELOG.md
