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
    # Noti Agent OS page
    page_id = os.getenv("NOTI_PAGE_ID")
    stream_db_id = os.getenv("KOAD_STREAM_DB_ID")
    
    if not page_id or not stream_db_id:
        print("Error: NOTI_PAGE_ID and KOAD_STREAM_DB_ID must be set.")
        sys.exit(1)
    
    blocks = [
        {
            "object": "block",
            "type": "heading_2",
            "heading_2": {
                "rich_text": [{"type": "text", "text": {"content": "COMMUNICATION PROTOCOLS (v1)"}}]
            }
        },
        {
            "object": "block",
            "type": "paragraph",
            "paragraph": {
                "rich_text": [
                    {
                        "type": "text", 
                        "text": {"content": "Noti and Koad (CLI) communicate via the "}
                    },
                    {
                        "type": "mention",
                        "mention": {
                            "type": "database",
                            "database": {"id": stream_db_id}
                        }
                    }
                ]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": "Noti check: Whenever active, Noti should check the stream for 'Unread' topics addressed to her or tagged 'Question'."}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": "Acknowledgment: When Noti acts on a stream item, she moves Status to 'Acknowledged' and posts a follow-up if needed."}}]
            }
        },
        {
            "object": "block",
            "type": "bulleted_list_item",
            "bulleted_list_item": {
                "rich_text": [{"type": "text", "text": {"content": "Pulse Posts: Koad CLI posts operational logs and facts to the stream as 'Log' types for Noti's ingestion."}}]
            }
        }
    ]
    
    append_blocks(page_id, blocks)
