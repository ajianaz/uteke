---
title: Rooms
---

# Rooms

Group related memories by context — meetings, projects, discussions.

## Create a Room

```bash
uteke room create "project-kickoff" --title "Project Kickoff"
```

## Add Memories to a Room

Memories are added to a room via the HTTP API or by using `--room` when storing:

```bash
# Via HTTP API
curl -X POST http://127.0.0.1:8767/room/remember \
  -H "Content-Type: application/json" \
  -d '{"room_id": "project-kickoff", "content": "Decided on PostgreSQL", "author": "alice"}'

# Or store normally, then link via room document
uteke room summary "project-kickoff"
```

## Semantic Recall Within a Room

```bash
uteke room recall "project-kickoff" --query "database decision"
```

## Generate a Structured Document

Combine room memories into a cohesive document:

> **Note:** The CLI command remains `uteke room document`, but the underlying HTTP API route has been renamed from `POST /room/document` to `POST /room/summary-document`.

```bash
uteke room document "project-kickoff"
```

## Get a Room Summary

```bash
uteke room summary "project-kickoff"
```

## Use Cases

- **Meeting notes** — create a room per meeting, add memory IDs from discussions
- **Project context** — group all project-related memories for easy recall
- **Research** — compile findings into a structured document via `room document`

## See Also

- [Multi-Agent Isolation](/multi-agent) — each agent can have its own rooms
- [CLI Reference — Room Commands](/cli-reference#room-commands) — full command reference
