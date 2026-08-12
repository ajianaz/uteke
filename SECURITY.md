# Security Policy

## Supported Versions

Uteke is under active development. We provide security fixes for the **latest release** and the `develop` branch.

| Version | Supported |
|---------|-----------|
| Latest release | ✅ |
| develop branch | ✅ |
| Older versions | ❌ |

## Reporting a Vulnerability

We take security seriously. If you discover a vulnerability, please follow responsible disclosure:

### How to Report

1. **Do NOT open a public GitHub issue.**
2. Email: **security@codecora.dev**
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### Response Timeline

| Step | Target |
|------|--------|
| Acknowledgment | 48 hours |
| Initial assessment | 5 business days |
| Fix or mitigation | 30 days (severity-dependent) |
| Public disclosure | After fix is released |

### Scope

In scope:
- Uteke core (`uteke`, `uteke-core`, `uteke-cli`, `uteke-mcp`, `uteke-server` crates)
- Official extensions (`hermes-memory-provider`, `pi-memory-provider`)
- Installation scripts (`install.sh`, `install.ps1`)
- Documentation site (if it leads to security misunderstandings)

Out of scope:
- Self-hosted configurations that expose ports to the public internet without auth
- Vulnerabilities in third-party dependencies (report upstream)
- Social engineering attacks

### Rewards

We are a small open-source project. While we cannot offer monetary bounties at this time, we will:

- Credit you in the security advisory (unless you prefer to remain anonymous)
- Give priority to any feature requests you submit
- Be genuinely grateful 🙏

## Security Measures

Uteke follows these security practices:

- **`unsafe_code = "forbid"`** — zero unsafe Rust blocks in the codebase
- **Input validation** — all API inputs sanitized (command injection prevention)
- **Checksum verification** — release binaries are checksum-verified before installation
- **Safe string operations** — `safe_truncate()` replaces all fixed-offset slicing
- **Transaction safety** — memory + tag dual-write uses single SQLite transaction
- **Namespace isolation** — recall respects namespace boundaries by default
- **Case-sensitive routing** — prevents data leakage across namespaces

## Dependency Security

We monitor dependencies via:
- `cargo audit` in CI pipeline
- Dependabot for automated dependency updates
- Manual review of all dependency additions

---

*Found a security issue? Email security@codecora.dev — not GitHub issues.*
