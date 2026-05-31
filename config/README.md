# KoadOS Configuration Directory

This directory contains the core configuration files for the KoadOS Citadel.

## Structure

- `defaults/`: **[Distribution Tracked]** Contains the canonical templates and default settings for the platform. These files are committed to the repository and should be used as the source of truth for all Citadels.
- `kernel.toml`: **[Instance Local]** The active configuration for this specific Citadel. This file is generated at bootstrap from `defaults/kernel.toml` and is gitignored to prevent deployment-specific settings (like local paths, ports, and agent selections) from leaking into the distribution.
- `redis.conf`: **[Instance Local]** The active Redis configuration, gitignored.
- `identities/`: **[Distribution Tracked]** Canonical KAI (KoadOS Agent Identity) definitions.
- `integrations/`: **[Distribution Tracked]** Third-party service integration schemas (Notion, Stripe, etc.).
- `interfaces/`: **[Distribution Tracked]** Runtime interface configurations (Claude, Gemini, etc.).

## Bootstrapping a New Citadel

When deploying a new KoadOS instance, the `install/bootstrap.sh` script will automatically copy the files from `defaults/` to the live location if they do not already exist.

To manually bootstrap:
```bash
cp -n config/defaults/kernel.toml config/kernel.toml
cp -n config/defaults/redis.conf config/redis.conf
```
