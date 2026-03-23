# Entrance Memory SQL Draft

> This is a migration draft for the future Entrance repository.
> It is intentionally written as a proposal document because the real `src-tauri/migrations` tree is still missing.

## 1. Goal

Translate the recovered NOTA memory model into an Entrance-owned SQL migration plan once the source repository is restored.

## 2. Candidate Tables

Core tables that now look stable enough to migrate:

- `instincts`
- `documents`
- `coffee_chats`
- `todos`
- `memory_fragments`
- `decisions`
- `visions`
- `memory_links`

## 3. Todo Reminder Fields

The recovered NOTA side now treats reminder metadata as first-class on `todos`.

Candidate columns:

```sql
due_on TEXT DEFAULT '';
remind_every_days INTEGER DEFAULT 0;
remind_next_on TEXT DEFAULT '';
last_reminded_at TEXT DEFAULT '';
reminder_status TEXT DEFAULT 'none';
```

This allows:

- todos with no reminder at all
- one-off due dates
- recurring reminder cadences
- explicit pause/done states for reminders

## 4. Graph Preservation

`memory_links` should survive migration as a first-class table rather than being dissolved into ad hoc join logic.

Candidate shape:

```sql
src_kind TEXT NOT NULL;
src_id INTEGER NOT NULL;
dst_kind TEXT NOT NULL;
dst_id INTEGER NOT NULL;
relation_type TEXT NOT NULL;
status TEXT NOT NULL DEFAULT 'active';
created_at TEXT NOT NULL DEFAULT (datetime('now'));
```

## 5. Proposal for Documents

`documents` already behaves like canonical text storage.

The likely Entrance direction is:

- keep canonical documents in DB
- generate file views or exports for human use
- evaluate a future `project_documents` split only after repo recovery and adjacent schema review

## 6. Migration Sequence

Suggested sequence after repo recovery:

1. create SQL migration files for current canonical tables
2. copy `.agents` recovery data into Entrance-owned tables and recovery documents
3. verify graph links and provenance survive import
4. keep markdown export as compatibility layer
5. keep `.agents` intact until the Entrance-backed copy is verified
6. after verification, treat `.agents` as historical recovery substrate rather than canonical truth

## 7. Open Cautions

- do not prematurely encode distributed-runtime assumptions
- do not conflate conflict with lifecycle
- do not lose reminder metadata during migration
- do not lose provenance links from recovered fragments
