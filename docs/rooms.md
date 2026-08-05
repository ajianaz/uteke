# Rooms — Shared Memory for Multi-Agent Collaboration

> **Other memory layers are single-player.** They store facts under a flat `user_id` and every recall is scoped to one agent's view. Uteke Rooms let multiple AI agents share a memory space — meeting notes, project decisions, architecture choices — searchable by everyone, attributed by author.

## What is a Room?

A Room is a named, shared memory context. Multiple agents can read and write to the same Room, and every memory carries author attribution so you always know who said what.

| Concept | Single-agent memory | Uteke Rooms |
|---|---|---|
| **Scope** | One agent, one namespace | Multiple agents, one shared space |
| **Attribution** | `namespace` only | `author` per memory |
| **Cross-search** | ❌ No | ✅ Any agent can recall across rooms |
| **Use case** | Personal notes | Team knowledge, multi-agent workflows |

## Quick Start

```bash
# Create a room
uteke room create "engineering" --description "Engineering team decisions"

# Remember into the room (author is set from config or --author flag)
uteke remember "Postgres migration deferred to Q4 due to testing capacity" \
  --room engineering \
  --author alice

# Recall from the room — any agent can do this
uteke recall "postgres migration" --room engineering

# List all rooms
uteke room list

# Get room summary (auto-generated overview of recent activity)
uteke room summary engineering
```

## Why Rooms Matter

### The Problem with Flat Namespaces

Traditional memory tools store everything under a flat `user_id`. This works for single-agent chatbots, but breaks down when:

- **Multiple agents work on the same project** — Agent A can't see Agent B's memories
- **A team shares an AI assistant** — everyone's memories are siloed
- **You want cross-agent knowledge transfer** — there's no shared space

### The Rooms Solution

Rooms create a shared memory layer that sits above individual agent namespaces:

```
┌─────────────────────────────────────────┐
│           Room: "engineering"           │
│                                         │
│  Alice: "Migrated to Postgres 16"       │
│  Bob:   "API rate limit set to 1000/m"  │
│  Hermes:"Deploy scheduled for Friday"   │
│                                         │
│  → Any agent can recall all entries     │
│  → Author attribution on every memory   │
└─────────────────────────────────────────┘
```

## Use Cases

### 1. Engineering Team Knowledge Base

```bash
# Alice stores a decision
uteke remember "We chose Redis for caching, not Memcached" \
  --room engineering --author alice --tags decision,infra

# Bob adds context
uteke remember "Redis cluster: 3 nodes, 2 replicas each" \
  --room engineering --author bob --tags infra,redis

# New team member's agent recalls the history
uteke recall "caching decision" --room engineering
```

### 2. Multi-Agent Project Coordination

```bash
# Hermes agent stores project context
uteke remember "Client demo moved to Thursday 2pm" \
  --room project-alpha --author hermes

# Claude agent picks it up
uteke recall "client demo" --room project-alpha
# → "Client demo moved to Thursday 2pm" (author: hermes)
```

### 3. Meeting Notes with Attribution

```bash
# Import meeting transcript with extraction
uteke import meeting-notes.txt \
  --room weekly-standup \
  --author alice \
  --extract

# Each extracted fact retains author attribution
uteke recall "action items" --room weekly-standup
```

## Room Management

| Command | Description |
|---|---|
| `uteke room create <name>` | Create a new room |
| `uteke room create <name> --description <text>` | Create with description |
| `uteke room list` | List all rooms |
| `uteke room summary <name>` | Auto-generated summary of room activity |
| `uteke room delete <name>` | Delete a room and its memories |

## API Reference

### Create Room

```bash
uteke room create <name> [--description <text>]
```

### Remember into Room

```bash
uteke remember "<content>" \
  --room <room-name> \
  --author <author-name> \
  [--tags tag1,tag2] \
  [--entity <entity>] \
  [--category <category>]
```

### Recall from Room

```bash
uteke recall "<query>" --room <room-name> [--limit 10]
```

### HTTP API

```http
POST /room/create
Content-Type: application/json

{"name": "engineering", "description": "Team decisions"}

POST /remember
Content-Type: application/json

{
  "content": "Postgres migration deferred to Q4",
  "room": "engineering",
  "author": "alice",
  "tags": ["decision", "infra"]
}

GET /recall?q=migration&room=engineering
```

## Room vs Namespace

| Aspect | Namespace | Room |
|---|---|---|
| **Purpose** | Agent isolation | Shared context |
| **Visibility** | Single agent | Multiple agents |
| **Attribution** | Implicit (namespace = agent) | Explicit (`--author` flag) |
| **Cross-search** | ❌ | ✅ |
| **Use together** | ✅ Namespaces + Rooms work in combination | |

Namespaces and Rooms are complementary. An agent has its own namespace for private memories, and can participate in multiple Rooms for shared knowledge.

## See Also

- [Quick Start](/getting-started) — Get up and running in 30 seconds
- [Multi-Agent](/multi-agent) — How multiple agents coexist with uteke
- [CLI Reference](/cli-reference) — Full command documentation
- [HTTP API](/api-reference) — REST API for server mode
