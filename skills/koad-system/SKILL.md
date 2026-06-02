---
name: koad-system
description: Use when starting, stopping, or restarting Citadel services, checking system health, triggering a save/backup, or recovering from a disconnected session.
---

# koad system

Core Citadel lifecycle and ops management. Required knowledge for any agent running infrastructure.

## Service Lifecycle

```bash
koad system start      # start Citadel kernel + CASS + all services
koad system stop       # graceful shutdown
koad system restart    # restart all services
koad system status     # real-time service telemetry
```

Run `koad system start` when CASS or Citadel show OFFLINE at boot.

## Health & Recovery

```bash
koad doctor            # comprehensive health check + self-healing sweep
koad system reconnect  # re-establish neural link after disconnection/reboot
koad system logs       # tail or filter KoadOS logs
```

Run `koad doctor` before reporting a service as broken. It self-heals many common issues.

## State & Safety

```bash
koad system save       # Sovereign Save Protocol — full state checkpoint
koad system backup     # manual memory sector backup
koad system lock <sector>    # acquire distributed lock
koad system unlock <sector>  # release distributed lock
```

Always run `save` before risky operations (migrations, bulk deletes, schema changes).

## Config & Auth

```bash
koad system config     # inspect or modify global config
koad system auth       # show active credentials and PAT mapping
koad system tokenaudit # 5-pass cognitive efficiency audit
```

## Destructive (confirm before running)

```bash
koad system scrub      # removes local state, logs, DBs — prep for distribution
```

**Warning:** `scrub` is irreversible. Run `save` first.

## Common Mistakes

| Mistake | Fix |
|---|---|
| Reporting CASS offline without trying to start | Run `koad system start` first |
| Skipping `save` before schema changes | Always checkpoint before risky ops |
| Using `restart` to fix config issues | Use `koad doctor` first — it self-heals |
