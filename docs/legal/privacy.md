---
title: Privacy Policy
description: How Uteke handles your data — it stays on your machine.
---

# Privacy Policy

**Last updated:** August 11, 2026

## The Short Version

Uteke is an offline-first application. **Your data never leaves your machine** unless you explicitly configure a remote server or cloud sync.

No personal data is collected. No telemetry runs by default. No accounts are required.

---

## 1. Data Collection

### What We Collect

**Nothing.** Uteke does not collect, transmit, or store any personal data, usage data, or memory content on any server controlled by CodeCora.

### What Stays on Your Machine

Everything:

| Data | Location | Purpose |
|------|----------|---------|
| Memories (content + embeddings) | `~/.uteke/` (or configured store path) | Your knowledge base |
| Vector index (HNSW) | `~/.uteke/index/` | Fast similarity search |
| SQLite database | `~/.uteke/uteke.db` | Metadata, tags, relationships |
| Configuration | `~/.uteke/config.toml` | Your settings |
| Embedding model cache | `~/.uteke/models/` | ONNX model (downloaded once) |

You have full control. Delete `~/.uteke/` and everything is gone.

---

## 2. Opt-In Telemetry

Telemetry is **disabled by default** and must be explicitly enabled by you.

If you choose to enable it, Uteke sends a minimal anonymous ping once per 24 hours containing **only**:

```json
{
  "anonymous_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "version": "0.13.1",
  "os": "linux",
  "arch": "aarch64",
  "memories_count_bucket": "100-1000",
  "event": "startup"
}
```

**What is NOT sent:**

- ❌ Memory content (never)
- ❌ Embedding vectors
- ❌ Your name, email, or identity
- ❌ File paths or system information
- ❌ Network information

The `anonymous_id` is a randomly generated UUID created during `uteke init`. It cannot be traced back to you.

To enable: `uteke config set telemetry.enabled true`

To disable: `uteke config set telemetry.enabled false` (default)

---

## 3. Network Activity

### Default Operation (Offline)

When running locally with default settings, Uteke makes **zero outbound network connections**. The only exception is the update checker, which:

- Runs once per 24 hours (cached)
- Sends a GET request to `https://api.github.com/repos/codecoradev/uteke/releases/latest`
- Transmits no data — only reads the latest release version
- Can be disabled: `uteke config set update_check.enabled false`

### Server Mode

If you run `uteke serve`, Uteke listens on `127.0.0.1:8767` by default (localhost only). It does not expose any port to the internet unless you explicitly configure it to.

### Remote Backends (Optional)

If you configure an external embedding provider (e.g., OpenAI, Ollama) or a remote Qdrant instance, data will be sent to that provider according to their privacy policy. This is entirely opt-in and configured by you.

---

## 4. Embedding Model

Uteke uses EmbeddingGemma 300M (quantized to INT4, ONNX format) for local embedding generation. The model:

- Runs entirely on your CPU
- Is downloaded once from HuggingFace during first use (~188MB)
- Does not make any subsequent network calls
- Processes your text locally — no data is sent to any API

---

## 5. Data When Using Third-Party Tools

If you use Uteke through an MCP-compatible tool (Claude, Cursor, Copilot, Hermes, etc.), that tool's privacy policy applies to its own operations. Uteke's storage remains local regardless of which tool connects to it.

---

## 6. Children's Privacy

Uteke is a developer tool. We do not knowingly collect any data from anyone, including children. If you believe a child has interacted with Uteke inappropriately, there's nothing to worry about — we don't collect any data.

---

## 7. Changes to This Policy

We will update this policy if Uteke's data practices change. Any material changes will be noted in the [CHANGELOG](https://github.com/codecoradev/uteke/blob/develop/CHANGELOG.md).

---

## 8. Contact

Questions about privacy?

- Open an issue: [github.com/codecoradev/uteke/issues](https://github.com/codecoradev/uteke/issues)
- Email: privacy@codecora.dev

---

*Uteke is built on a simple principle: your data is yours. We can't access it, and we don't want to.*
