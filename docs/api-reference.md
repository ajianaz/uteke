---
title: HTTP API Reference
---

# HTTP API Reference

Complete reference for all uteke-server HTTP endpoints. Version **0.10.1**.

All endpoints accept and return JSON (`Content-Type: application/json`). POST/PUT/DELETE bodies are JSON; GET endpoints use query parameters.

## Base URL

```
http://localhost:8767
```

Default port is `8767`. Override with `--port` flag or `UTEKE_PORT` env var.

## API Versioning

All routes support optional `/api/v1` or `/api/v2` prefix for version negotiation ([#737](https://github.com/codecoradev/uteke/issues/737)):

| Version | Format | Description |
|---------|--------|-------------|
| `v1` | Flat recall results | v0.7.x compatible (`[{id, content, score, ...}]`) |
| `v2` | Wrapped `UnifiedSearchResult` | v0.8.x+ (unified search with metadata) |
| *(none)* | Latest (`v2`) | Unversioned routes alias to latest |

```bash
# Versioned
POST /api/v2/recall
# Unversioned (aliases to v2)
POST /recall
```

---

## Health & Stats

### GET /health

Server health check. Returns version, memory count, and supported API versions.

**Response:**
```json
{
  "status": "ok",
  "version": "0.10.1",
  "memories": 148,
  "namespaces": 3,
  "api_versions": ["v1", "v2"],
  "api_latest": "v2"
}
```

### GET /stats

Global statistics (namespaces, memory counts, tag counts).

**Response:**
```json
{
  "namespaces": ["default", "cto", "cmo"],
  "total_memories": 148,
  "by_namespace": { "default": 50, "cto": 80, "cmo": 18 }
}
```

### POST /stats

Statistics for a specific namespace.

**Request:**
```json
{ "namespace": "default" }
```

---

## Memory Operations

### POST /remember

Store a new memory with automatic embedding generation.

**Request:**
```json
{
  "content": "Uteke uses SQLite + ONNX embeddings",
  "tags": ["architecture", "uteke"],
  "namespace": "default",
  "type": "fact",
  "entity": "uteke",
  "category": "architecture",
  "metadata": { "project": "uteke" },
  "source": "docs",
  "source_type": "user",
  "valid_from": "2026-01-01T00:00:00Z",
  "valid_until": null,
  "detect_contradiction": false
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `content` | string | ✅ | — | Memory content text |
| `tags` | string[] | ❌ | `[]` | Tags for filtering |
| `namespace` | string | ❌ | `default` | Namespace isolation |
| `type` | string | ❌ | `null` | Memory type (fact, procedure, preference, decision, etc.) |
| `entity` | string | ❌ | `null` | Entity name (stored as metadata) |
| `category` | string | ❌ | `null` | Category (stored as metadata) |
| `metadata` | object | ❌ | `null` | Extra key-value pairs |
| `source` | string | ❌ | `null` | Source provenance |
| `source_type` | string | ❌ | `user` | Source type |
| `valid_from` | string | ❌ | `null` | RFC3339 timestamp |
| `valid_until` | string | ❌ | `null` | RFC3339 timestamp |
| `detect_contradiction` | bool | ❌ | `false` | Check for contradictions |

### POST /recall

Semantic recall — search memories by meaning using embeddings.

**Request:**
```json
{
  "query": "how does uteke store data",
  "limit": 5,
  "tags": [],
  "namespace": "default",
  "entity": null,
  "category": null,
  "min_score": 0.0,
  "strict": false,
  "at": null,
  "search_type": "all",
  "enrich": false
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `query` | string | ✅ | — | Semantic search query |
| `limit` | int | ❌ | `5` | Max results |
| `tags` | string[] | ❌ | `[]` | Filter by tags |
| `namespace` | string | ❌ | `null` | Namespace filter |
| `entity` | string | ❌ | `null` | Filter by entity metadata |
| `category` | string | ❌ | `null` | Filter by category metadata |
| `min_score` | float | ❌ | `0.0` | Minimum similarity score |
| `strict` | bool | ❌ | `false` | Use strict threshold (0.5) |
| `at` | string | ❌ | `null` | Time-travel: RFC3339 timestamp |
| `search_type` | string | ❌ | `all` | `all`, `memory`, or `doc` |
| `enrich` | bool | ❌ | `false` | Enrich with cross-entity links |

### POST /search

Fast keyword/FTS search (non-semantic).

**Request:**
```json
{
  "query": "sqlite",
  "limit": 10,
  "tags": [],
  "namespace": "default"
}
```

### POST /list

List memories with pagination and optional tag filter.

**Request:**
```json
{
  "tag": "architecture",
  "limit": 5,
  "offset": 0,
  "namespace": "default",
  "at": null
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `tag` | string | ❌ | `null` | Filter by tag |
| `limit` | int | ❌ | `5` | Page size |
| `offset` | int | ❌ | `0` | Pagination offset |
| `namespace` | string | ❌ | `null` | Namespace filter |
| `at` | string | ❌ | `null` | Time-travel timestamp |

### DELETE /forget

Delete a memory by ID.

**Query params:** `?id=<uuid>`

```bash
DELETE /forget?id=550e8400-e29b-41d4-a716-446655440000
```

### GET /memory

Get a single memory by ID.

**Query params:** `?id=<uuid>`

### PUT /memory

Update a memory ([#659](https://github.com/codecoradev/uteke/issues/659)). Triggers embedding regeneration if content changes.

**Request:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "content": "updated content",
  "tags": ["new-tag"],
  "metadata": { "key": "value" },
  "importance": 0.8,
  "pinned": true,
  "memory_type": "decision"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | ✅ | Memory UUID |
| `content` | string | ❌ | New content (triggers re-embedding) |
| `tags` | string[] | ❌ | Replace tags |
| `metadata` | object | ❌ | Replace metadata |
| `importance` | float | ❌ | Importance score (0.0–1.0) |
| `pinned` | bool | ❌ | Pinned state |
| `memory_type` | string | ❌ | Memory type |

### POST /memory/pin

Set pinned state on a memory ([#660](https://github.com/codecoradev/uteke/issues/660)).

**Request:**
```json
{ "id": "550e8400-...", "pinned": true }
```

### POST /memory/importance

Set importance score on a memory.

**Request:**
```json
{ "id": "550e8400-...", "importance": 0.9 }
```

### POST /memory/feedback

Submit feedback for trust scoring ([#718](https://github.com/codecoradev/uteke/issues/718)).

**Request:**
```json
{ "id": "550e8400-...", "feedback": "helpful" }
```

`feedback` must be `"helpful"` or `"unhelpful"`.

### POST /memory/doc-refs

Get document references linked to a memory ([#689](https://github.com/codecoradev/uteke/issues/689)).

**Request:**
```json
{ "memory_id": "550e8400-..." }
```

---

## Pin Operations

### POST /pin

Pin a memory by ID.

**Request:**
```json
{ "id": "550e8400-..." }
```

### POST /unpin

Unpin a memory by ID.

**Request:**
```json
{ "id": "550e8400-..." }
```

---

## Namespace & Tags

### GET /namespaces

List all namespaces.

**Query params:** `?memory_count=true` (include memory counts per namespace)

### GET /tags

List all tags with usage counts.

**Query params:** `?namespace=<name>`

### POST /tags/rename

Rename a tag across all memories.

**Request:**
```json
{ "old": "arch", "new": "architecture", "namespace": "default" }
```

### POST /tags/delete

Delete a tag from all memories.

**Request:**
```json
{ "tag": "deprecated", "namespace": "default" }
```

---

## Recent & Graph

### GET /recent

Get recently accessed memories.

**Query params:** `?limit=10&namespace=default`

### GET /graph

Get relationship graph for an entity.

**Query params:** `?entity=<name>&depth=3`

### POST /graph/edge

Create a relationship edge between entities.

**Request:**
```json
{
  "source": "uteke",
  "target": "sqlite",
  "edge_type": "uses",
  "weight": 1.0
}
```

### DELETE /graph/edge

Delete a relationship edge.

**Query params:** `?source=<name>&target=<name>&edge_type=<type>`

---

## Rooms

Rooms are collaborative memory spaces for multi-agent discussions.

### POST /room/create

Create a new room.

**Request:**
```json
{ "room_id": "project-discussion", "title": "Project Discussion", "namespace": "default" }
```

### POST /room/remember

Store a memory and link it to a room.

**Request:**
```json
{
  "room_id": "project-discussion",
  "content": "Decision: use SQLite for storage",
  "author": "cto",
  "tags": ["decision"],
  "role": "lead"
}
```

### GET /room/memories

List memories in a room (chronological order).

**Query params:** `?room_id=<id>&author=<name>&limit=10`

### POST /room/recall

Semantic recall within a room. Query is **optional** — when omitted or empty, falls back to chronological recall ([#785](https://github.com/codecoradev/uteke/issues/785)).

**Request (semantic — with query):**
```json
{
  "room_id": "project-discussion",
  "query": "database decision",
  "limit": 5,
  "author": null,
  "min_score": 0.0
}
```

**Request (chronological fallback — no query):**
```json
{
  "room_id": "project-discussion",
  "limit": 10,
  "author": "cto"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `room_id` | string | ✅ | — | Room identifier |
| `query` | string | ❌ | `null` | Semantic query. When empty/absent, falls back to chronological |
| `limit` | int | ❌ | `5` | Max results |
| `author` | string | ❌ | `null` | Filter by author |
| `min_score` | float | ❌ | `0.0` | Minimum similarity (semantic mode only) |

### POST /room/stats

Get room statistics (memory count, participant count, participants list, last activity). Excludes deprecated memories ([#784](https://github.com/codecoradev/uteke/issues/784)).

**Request:**
```json
{ "room_id": "project-discussion" }
```

**Response:**
```json
{
  "memory_count": 12,
  "participant_count": 3,
  "participants": ["cto", "cmo", "coo"],
  "last_activity": "2026-07-27T10:30:00Z"
}
```

### POST /room/summary

Get or generate a room summary.

**Request:**
```json
{ "room_id": "project-discussion" }
```

### POST /room/summary-document

Get the summary document for a room.

**Request:**
```json
{ "room_id": "project-discussion" }
```

### POST /room/document

Get the document associated with a room.

**Request:**
```json
{ "room_id": "project-discussion" }
```

### POST /room/document/list

List documents linked to a room.

**Request:**
```json
{ "room_id": "project-discussion" }
```

### PUT /room/document/add

Link a document to a room.

**Request:**
```json
{ "room_id": "project-discussion", "doc_id": "doc-uuid" }
```

### DELETE /room/document/remove

Unlink a document from a room.

**Query params:** `?room_id=<id>&doc_id=<uuid>`

### POST /doc/room/list

List rooms linked to a document.

**Request:**
```json
{ "doc_id": "doc-uuid" }
```

### GET /room/list

List all rooms.

**Query params:** `?namespace=<name>`

### DELETE /room/delete

Delete a room. Cascades `room_memories` links (memories themselves survive).

**Query params:** `?room_id=<id>`

---

## Documents

Documents are structured content with hierarchical parent-child relationships.

### POST /doc/create

Create a new document.

**Request:**
```json
{
  "slug": "architecture-overview",
  "title": "Architecture Overview",
  "content": "# Architecture\n\n...",
  "tags": ["architecture"],
  "parent": null
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `slug` | string | ✅ | — | URL-safe identifier |
| `title` | string | ❌ | `null` | Display title |
| `content` | string | ✅ | — | Document content (markdown) |
| `tags` | string[] | ❌ | `[]` | Tags |
| `parent` | string | ❌ | `null` | Parent doc slug (for hierarchy) |

### POST /doc/get

Get a document by ID or slug.

**Request:**
```json
{ "id": "uuid-or-null", "slug": "architecture-overview" }
```

Provide either `id` or `slug`.

### POST /doc/update

Update a document.

**Request:**
```json
{
  "id": null,
  "slug": "architecture-overview",
  "title": "Updated Title",
  "content": "updated content",
  "tags": ["architecture", "updated"],
  "metadata": null
}
```

### POST /doc/list

List documents.

**Request:**
```json
{ "limit": 1000, "roots_only": false, "parent": null }
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `limit` | int | `1000` | Max results (high default for full tree) |
| `roots_only` | bool | `false` | Only top-level docs (no parent) |
| `parent` | string | `null` | Filter by parent slug |

### POST /doc/search

Search documents.

**Request:**
```json
{ "query": "storage", "limit": 5, "mode": "hybrid" }
```

`mode`: `hybrid` (default), `fts`, or `semantic`.

### POST /doc/move

Move a document to a new parent.

**Request:**
```json
{ "id": null, "slug": "child-doc", "new_parent": "new-parent-slug" }
```

### DELETE /doc/delete

Delete a document.

**Query params:** `?id=<uuid>` or `?slug=<slug>`

### POST /doc/mem-refs

Get memory references linked to a document ([#689](https://github.com/codecoradev/uteke/issues/689)).

**Request:**
```json
{ "doc_id": "doc-uuid" }
```

---

## Advanced

### POST /context

Get contextual memories for a given input.

**Request:** Arbitrary JSON object.

### POST /dream

Trigger dream/consolidation process.

**Request:** Arbitrary JSON object.

### POST /mcp

MCP (Model Context Protocol) JSON-RPC endpoint. See [MCP Server docs](/mcp) for details.

---

## Error Responses

All errors return HTTP 4xx/5xx with a JSON error body:

```json
{ "error": "Description of what went wrong" }
```

| Status | Meaning |
|--------|---------|
| 400 | Bad request — invalid JSON, missing required field, validation error |
| 401 | Unauthorized — missing or invalid API key |
| 404 | Not found — resource doesn't exist |
| 413 | Payload too large — exceeds `MAX_PAYLOAD_SIZE` |
| 500 | Internal server error — unexpected failure |

---

## Authentication

If `UTEKE_API_KEY` is set, all endpoints require an `Authorization: Bearer <key>` header. When unset, the server runs in open mode (no auth).

```bash
# With auth
curl -H "Authorization: Bearer your-secret-key" http://localhost:8767/stats
```

---

## CORS

All responses include CORS headers (`Access-Control-Allow-Origin: *`) for browser-based clients. Preflight `OPTIONS` requests are handled automatically.

---

## See Also

- [CLI Reference](/cli-reference) — uteke CLI commands
- [Configuration](/configuration) — server configuration options
- [Rooms](/rooms) — room feature guide
- [MCP Server](/mcp) — MCP protocol integration
- [TLS & Reverse Proxy](/tls) — production deployment
