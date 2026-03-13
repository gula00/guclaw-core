# gula-db-core

SQLite-first Rust database core exposed to Node.js via N-API.

This crate is used as the native storage engine for Gula desktop runtime.
It focuses on predictable local persistence, schema evolution, and fast read paths.

## Scope

- Thread and message storage (CRUD + version metadata)
- Provider/settings/workspace/prompt state
- FTS helpers (`messages_fts`) and maintenance APIs
- Gallery cache storage and metadata extraction
- Memory schema helpers and memory metadata utilities
- N-API bindings for Node/Electron integration

## Project Layout

- `src/lib.rs`: `open_db`, `DbHandle`, connection lifecycle
- `src/types.rs`: N-API request/response structs
- `src/ops/*.rs`: domain operations split by concern
- `build.rs`: napi build hook

## Build

Requirements:

- Rust stable toolchain
- Node.js 18+
- npm

Commands:

```bash
# Rust compile check
cargo build

# Build Node addon (.node)
npm install
npm run build:napi
```

Release build:

```bash
npm run build:napi:release
```

## Runtime Integration

Set the DB backend and addon path in the host app:

```bash
GULA_DB_BACKEND=rust
GULA_DB_NATIVE_PATH=/absolute/path/to/index.node
GULA_DB_RUST_DEBUG=1
```

Notes:

- `GULA_DB_RUST_DEBUG` is optional and enables per-call logs.
- Use absolute `GULA_DB_NATIVE_PATH` in production launch scripts.

## Minimal Usage

```js
const native = require('./index.node')
const open = native.open_db || native.openDb
const db = open('/path/to/chat_threads.db')

db.ensureSchema?.()
db.runPostSchemaMigrations?.()

const thread = db.getThread?.('thread-id')
db.close()
```

## API Surface

The API is intentionally broad and grouped by domain in `src/ops`:

- schema and migrations
- thread/message
- provider/settings
- workspace/mcp/prompt/skill/label
- cache (fts/gallery/channel/model capabilities)
- memory helpers

For exact method signatures, use generated typings from N-API build output.

## Open Source Checklist

- Add `repository`, `homepage`, and `bugs` fields in `package.json`
- Decide versioning policy for schema migrations
- Keep `.node` binaries out of source control for tagged releases
