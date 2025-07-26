**Purpose**
- Guide AI/code agents contributing to this repo with clear, pragmatic conventions derived from ~/.claude/CLAUDE.md and tailored to SNAS (Rust, NATS, workspace).

**Scope**
- Applies to the entire repository. More‑specific instructions in nested AGENTS.md files would override this, but none exist today.

**Philosophy**
- Incremental progress over big bangs; prefer small, compiling changes with passing tests.
- Learning from existing code; mimic current patterns in `snas-lib` and binaries.
- Pragmatic over dogmatic; choose the boring solution that’s easy to maintain.
- Clear intent over clever code; if it needs a long explanation, simplify it.

**Project Overview**
- Rust 2021 workspace with two binaries and a shared library:
  - `bin/server/main.rs` → `snas-server` (runs NATS and socket API servers)
  - `bin/cli/main.rs` → `snas` (reserved CLI binary)
  - `crates/snas-lib` → core types, storage, servers, clients
- External dependency: a reachable NATS server for most integration and e2e tests.

**Environment & Tooling**
- Build: `cargo build`
- Test (unit + integration): `cargo test`
- Formatting: `cargo fmt --all`
- Linting: `cargo clippy --all-targets -- -D warnings`
- NATS for tests: ensure a local NATS with JetStream is running at `127.0.0.1:4222`.
  - Quick start (Docker): `docker run --rm -p 4222:4222 -p 8222:8222 nats:latest -js`
  - Or install locally: `nats-server -js`
- Nix/direnv supported: `nix develop` and `.envrc` are present; prefer using them if available.
- Version control: prefer `jj` (Jujutsu) for history inspection; fall back to `git` if needed.

**Process**
- Plan briefly before coding. For multi-step work, create an implementation plan (kept in your agent context) with 3–5 stages and testable outcomes.
- Default flow: understand → write or adapt tests (red) → implement minimal code (green) → refactor with tests passing → commit.
- Maximum 3 failed attempts on the same issue; then stop, document findings, and propose alternatives.

**Code Standards**
- Small, single-responsibility functions and modules.
- Avoid premature abstractions; compose via traits and generics as per existing patterns.
- Prefer explicit data flow and dependency injection over globals/singletons.
- Error handling: return rich context with `anyhow`/`thiserror`; never silently swallow errors; log via `tracing`.
- Public API in `crates/snas-lib` is semi-stable; do not change types or signatures unless required by the feature/bugfix. If you must, update all call sites and tests.
- Keep changes minimal and focused; do not fix unrelated issues in the same patch.

**Testing**
- Run `cargo test` locally. Many integration tests require NATS at `127.0.0.1:4222`.
- Tests are deterministic; prefer behavior-focused assertions. Follow established helpers in `tests/helpers.rs`.
- Socket tests use temporary directories and Unix Domain Sockets; keep platform specifics guarded behind `#[cfg(unix)]` as in current code.

**Runtime & Binaries**
- Run server: `cargo run --bin snas-server -- --help`
  - Common flags: `--nats-server 127.0.0.1 --nats-port 4222 --kv-bucket snas`
  - Enable NATS APIs: `--admin-nats` and/or `--user-nats` with optional topic prefixes.
  - Socket API (unix): `--user-socket --socket-file /path/to/sock` (see `DEFAULT_SOCKET_PATH`).
- The CLI binary `snas` is currently a placeholder; keep it compiling while adding features incrementally.

**Repo Conventions**
- Workspace membership is defined in `Cargo.toml` `[workspace]`; update it if you add crates.
- Keep tracing configuration consistent with `bin/server/main.rs` (JSON vs pretty based on flag and TTY).
- Follow existing module layout and visibility rules in `snas-lib` (e.g., `handlers`, `servers`, `storage`, `types`).

**Quality Gates**
- Code compiles and passes all tests (`cargo test`).
- No clippy warnings (`cargo clippy -- -D warnings`).
- Formatted with `cargo fmt`.
- Clear commit messages explaining why the change exists.
- No stray TODOs without context or tracking issues.

**When Stuck**
- After three attempts: write down what failed, error messages, and suspected causes; propose 2–3 alternative approaches referencing similar code in this repo.
- Reevaluate abstraction level; consider simpler or more incremental changes.

**Agent-specific Guidance**
- Prefer surgical patches. Touch only files necessary for the change.
- Read files in small chunks; avoid mass rewrites.
- Use existing crates and dependencies; do not introduce new libraries unless strongly justified.
- Validate locally by running the narrowest relevant tests first (e.g., `cargo test -p snas -- tests::storage`).
- If modifying networked behavior (NATS topics, socket protocol), update `socket_protocol.md` and any relevant tests.

**Safety Notes**
- Admin and user NATS APIs are disabled by default; be explicit when enabling in tests or examples.
- For production, the KV bucket should be provisioned with proper replication; tests default to in-memory storage.

**Contact & Ownership**
- Primary author: listed in `Cargo.toml:authors`.
- If a change materially alters API or behavior, note it in `README.md` and consider adding brief usage examples.

**Quick Checklists**
- Implementation
  - Changes are minimal and purposeful
  - Public API stability considered
  - Error paths exercised in tests
- Verification
  - `cargo fmt && cargo clippy -- -D warnings`
  - `cargo test` with a local NATS server
  - Binaries start and log correctly for basic flags

