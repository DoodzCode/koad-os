#!/usr/bin/env python3
import os
import sys
import requests
import json
import argparse
from datetime import datetime

DATABASE_ID = os.getenv("KOAD_STREAM_DB_ID")

def get_headers():
    token = os.getenv('NOTION_TOKEN') or os.getenv('NOTION_PAT')
    if not token:
        print("Error: NOTION_TOKEN (or NOTION_PAT) environment variable not set.")
        sys.exit(1)
    if not DATABASE_ID:
        print("Error: KOAD_STREAM_DB_ID environment variable not set.")
        sys.exit(1)
    return {
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json",
        "Notion-Version": "2022-06-28"
    }

def post_message(topic, message, author="Koad", msg_type="Log"):
    url = "https://api.notion.com/v1/pages"
    payload = {
        "parent": {"database_id": DATABASE_ID},
        "properties": {
            "Topic": {"title": [{"text": {"content": topic}}]},
            "Author": {"select": {"name": author}},
            "Type": {"select": {"name": msg_type}},
            "Status": {"select": {"name": "Unread"}}
        },
        "children": [
            {
                "object": "block",
                "type": "paragraph",
                "paragraph": {
                    "rich_text": [{"type": "text", "text": {"content": message}}]
                }
            }
        ]
    }
    response = requests.post(url, headers=get_headers(), json=payload)
    if response.status_code == 200:
        print(f"Successfully posted to Koad Stream: {topic}")
    else:
        print(f"Error posting to Notion: {response.status_code}")
        print(response.text)

def list_messages(limit=5):
    url = f"https://api.notion.com/v1/databases/{DATABASE_ID}/query"
    payload = {
        "page_size": limit,
        "sorts": [{"property": "Created Time", "direction": "descending"}]
    }
    response = requests.post(url, headers=get_headers(), json=payload)
    if response.status_code == 200:
        results = response.json().get("results", [])
        if not results:
            print("No messages found in Koad Stream.")
            return
        
        print(f"\n--- Koad Stream (Last {len(results)}) ---")
        for page in results:
            props = page["properties"]
            topic_list = props["Topic"]["title"]
            topic = topic_list[0]["plain_text"] if topic_list else "(No Topic)"
            author = props["Author"]["select"]["name"] if props["Author"]["select"] else "Unknown"
            msg_type = props["Type"]["select"]["name"] if props["Type"]["select"] else "Unknown"
            status = props["Status"]["select"]["name"] if props["Status"]["select"] else "Unknown"
            created = props["Created Time"]["created_time"]
            
            print(f"[{created}] {author} | {msg_type} | {status}")
            print(f"Topic: {topic}")
            print("-" * 30)
    else:
        print(f"Error querying database: {response.status_code}")
        print(response.text)

def main():
    parser = argparse.ArgumentParser(description="Koad Stream Notion Interface")
    subparsers = parser.add_subparsers(dest="command")

    # Post command
    post_parser = subparsers.add_parser("post")
    post_parser.add_argument("topic", help="Topic of the message")
    post_parser.add_argument("message", help="Content of the message")
    post_parser.add_argument("--author", default="Koad", choices=["Koad", "Noti", "Ian"])
    post_parser.add_argument("--type", default="Log", choices=["Log", "Question", "Decision", "Alert"])

    # List command
    list_parser = subparsers.add_parser("list")
    list_parser.add_argument("--limit", type=int, default=5, help="Number of messages to show")

    args = parser.parse_args()

    if args.command == "post":
        post_message(args.topic, args.message, args.author, args.type)
    elif args.command == "list":
        list_messages(args.limit)
    else:
        parser.print_help()

if __name__ == "__main__":
    main()
