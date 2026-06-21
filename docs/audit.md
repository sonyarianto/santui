# Architecture Audit

Generated 2026-06-20 from a comprehensive codebase review.

## Low — polish

- [ ] **No security/capability model between plugins** — IPC plugins have full system access (filesystem, network, etc.). No sandboxing or permission system. Low priority because plugins are opt-in and typically trusted.
