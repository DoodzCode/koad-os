import os
import requests
import json
import sys

def get_page_children(page_id):
    token = os.getenv('NOTION_TOKEN') or os.getenv('NOTION_PAT')
    if not token:
        print("Error: NOTION_TOKEN environment variable not set.")
        sys.exit(1)
    headers = {
        "Authorization": f"Bearer {token}",
        "Notion-Version": "2022-06-28"
    }
    url = f"https://api.notion.com/v1/blocks/{page_id}/children"
    response = requests.get(url, headers=headers)
    if response.status_code == 200:
        return response.json().get("results", [])
    return []

def archive_block(block_id):
    token = os.getenv('NOTION_TOKEN') or os.getenv('NOTION_PAT')
    headers = {
        "Authorization": f"Bearer {token}",
        "Notion-Version": "2022-06-28"
    }
    url = f"https://api.notion.com/v1/blocks/{block_id}"
    requests.delete(url, headers=headers)

def append_blocks(page_id, blocks):
    token = os.getenv('NOTION_TOKEN') or os.getenv('NOTION_PAT')
    headers = {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json",
        "Notion-Version": "2022-06-28"
    }
    url = f"https://api.notion.com/v1/blocks/{page_id}/children"
    payload = {"children": blocks}
    response = requests.patch(url, headers=headers, json=payload)
    return response.status_code

if __name__ == "__main__":
    page_id = os.getenv("KOAD_PAGE_ID")
    stream_db_id = os.getenv("KOAD_STREAM_DB_ID")
    noti_page_id = os.getenv("NOTI_PAGE_ID")
    projects_db_id = os.getenv("PROJECTS_DB_ID")
    memories_db_id = os.getenv("MEMORIES_DB_ID")
    
    if not page_id:
        print("Error: KOAD_PAGE_ID must be set.")
        sys.exit(1)
    
    # 1. Archive existing content
    print(f"Archiving old content for page {page_id}...")
    children = get_page_children(page_id)
    for child in children:
        archive_block(child["id"])
    
    # 2. Define new content based on koad.json v2.3
    blocks = [
        {
            "object": "block",
            "type": "heading_1",
            "heading_1": {
                "rich_text": [{"type": "text", "text": {"content": "KoadOS Core Contract (v2.3)"}}]
            }
        },
        {
            "object": "block",
            "type": "callout",
            "callout": {
                "rich_text": [{"type": "text", "text": {"content": "Identity: Principal Systems & Operations Engineer\\nAgnostic AI persona optimized for simplicity, reliability, and script-driven automation."}}],
                "icon": {"type": "emoji", "emoji": "\ud83e\uddd1\u200d\ud83d\ude80"},
                "color": "blue_background"
            }
        },
        {
            "object": "block",
            "type": "heading_2",
            "heading_2": {
                "rich_text": [{"type": "text", "text": {"content": "The Prime Directives"}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": "Simplicity over complexity."}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": "Plan before build."}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": "Native tech focus."}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": "Programmatic-first communication."}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": "Sanctuary Rule: Developer agents only touch project files & docs."}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": "Rust Stack: Prioritize cargo-llvm-cov, ratatui, cargo-doc, and cargo-deadlinks."}}]
            }
        },
        {
            "object": "block",
            "type": "heading_2",
            "heading_2": {
                "rich_text": [{"type": "text", "text": {"content": "Operational Infrastructure"}}]
            }
        },
        {
            "object": "block",
            "type": "code",
            "code": {
                "language": "yaml",
                "rich_text": [{"type": "text", "text": {"content": f"binary: ~/.koad-os/bin/koad\\nmemory: ~/.koad-os/koad.db\\nconfig: ~/.koad-os/koad.json\\nstream: {stream_db_id or 'NOT_SET'}"}}]
            }
        },
        {
            "object": "block",
            "type": "heading_2",
            "heading_2": {
                "rich_text": [{"type": "text", "text": {"content": "Communication Protocols"}}]
            }
        },
        {
            "object": "block",
            "type": "paragraph",
            "paragraph": {
                "rich_text": [
                    {"type": "text", "text": {"content": "All asynchronous delegation between Koad (CLI) and Noti (Notion AI) occurs in the "}},
                    {"type": "mention", "mention": {"type": "database", "database": {"id": stream_db_id or '00000000-0000-0000-0000-000000000000'}}}
                ]
            }
        },
        {
            "object": "block",
            "type": "heading_2",
            "heading_2": {
                "rich_text": [{"type": "text", "text": {"content": "Sync Index"}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": f"koad: {page_id}"}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": f"stream: {stream_db_id or 'NOT_SET'}"}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": f"noti: {noti_page_id or 'NOT_SET'}"}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": f"projects: {projects_db_id or 'NOT_SET'}"}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": f"memories: {memories_db_id or 'NOT_SET'}"}}]
            }
        }
    ]
    
    # Note: Adding table rows requires a separate call to children of the table block. 
    # For now, I will append the main blocks.
    
    print("Appending new content...")
    status = append_blocks(page_id, blocks)
    if status == 200:
        print("Koad page refreshed successfully.")
    else:
        print(f"Failed to append blocks: {status}")
