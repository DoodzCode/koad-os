import os
import requests
import json
import sys

def append_blocks(page_id, blocks):
    token = os.getenv('NOTION_TOKEN') or os.getenv('NOTION_PAT')
    if not token:
        print("Error: NOTION_TOKEN environment variable not set.")
        sys.exit(1)
    headers = {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json",
        "Notion-Version": "2022-06-28"
    }
    url = f"https://api.notion.com/v1/blocks/{page_id}/children"
    payload = {"children": blocks}
    response = requests.patch(url, headers=headers, json=payload)
    if response.status_code == 200:
        print("Successfully appended blocks.")
    else:
        print(f"Error: {response.status_code}")
        print(response.text)

if __name__ == "__main__":
    # Koad page
    page_id = os.getenv("KOAD_PAGE_ID")
    stream_db_id = os.getenv("KOAD_STREAM_DB_ID")
    
    if not page_id or not stream_db_id:
        print("Error: KOAD_PAGE_ID and KOAD_STREAM_DB_ID must be set.")
        sys.exit(1)
    
    blocks = [
        {
            "object": "block",
            "type": "heading_3",
            "heading_3": {
                "rich_text": [{"type": "text", "text": {"content": "DELEGATION STREAM (CLI \u2194 Noti)"}}]
            }
        },
        {
            "object": "block",
            "type": "paragraph",
            "paragraph": {
                "rich_text": [
                    {
                        "type": "text", 
                        "text": {"content": "Koad uses the "}
                    },
                    {
                        "type": "mention",
                        "mention": {
                            "type": "database",
                            "database": {"id": stream_db_id}
                        }
                    },
                    {
                        "type": "text",
                        "text": {"content": " to broadcast operational state and delegate admin tasks to Noti."}
                    }
                ]
            }
        },
        {
            "object": "block",
            "type": "code",
            "code": {
                "language": "bash",
                "rich_text": [
                    {"type": "text", "text": {"content": "# Post a log\\nkoad stream post \"Sync Complete\" \"All local facts synced to Airtable.\"\\n\\n# Ask Noti a question\\nkoad stream post \"Invoice Audit\" \"Noti, please check for unpaid MemberPlanet invoices from last week.\" --type Question"}}
                ]
            }
        }
    ]
    
    append_blocks(page_id, blocks)
