# KoadOS Gemini Bootstrap

## Notion MCP Integration
The Notion CLI commands (`koad stream`, `koad sync notion`) are now reconfigured to prefer the **Notion MCP Connection**.
- When asked to post to the Koad Stream, use the `API-post-page` or `API-patch-block-children` tools.
- Koad Stream Database ID: `310fe8ec-ae8f-80ba-9cbb-f31731d396d4`
- Use the `notion.index` in `koad.json` to find other relevant IDs.

## Core Directives
1. **Tool-First**: Prioritize MCP tools over shell scripts for external integrations.
2. **Consolidated Sync**: Use `koad sync notion` to trigger a multi-database pass instead of manual querying.
3. **Context Persistence**: Always run `koad boot` at session start.
4. **Fact Harvesting**: Use `koad saveup` to capture learnings.
5. **Surgical Discovery**: Prioritize `fdfind` (via `fd`) for locating files during research. Use `--extension` or `--type` flags to minimize noise and improve discovery speed.
6. **Token Conservation**: Actively minimize context noise. If a requested feature or operation significantly increases token usage (e.g., massive file injections, recursive deep-scans), propose a lower-cost alternative or seek explicit Partner approval before proceeding.
