import os
import requests
import json
import sys

def get_page_content(page_id):
    token = os.getenv('NOTION_TOKEN') or os.getenv('NOTION_PAT')
    if not token:
        print("Error: NOTION_TOKEN environment variable not set.")
        sys.exit(1)
    headers = {
        "Authorization": f"Bearer {token}",
        "Notion-Version": "2022-06-28"
    }
    response = requests.get(f"https://api.notion.com/v1/blocks/{page_id}/children", headers=headers)
    if response.status_code == 200:
        return response.json()
    else:
        print(f"Error: {response.status_code}")
        print(response.text)
        return None

if __name__ == "__main__":
    page_id = os.getenv('KOAD_PAGE_ID')
    if not page_id:
        print("Error: KOAD_PAGE_ID must be set.")
        sys.exit(1)
    content = get_page_content(page_id)
    if content:
        print(json.dumps(content, indent=2))
