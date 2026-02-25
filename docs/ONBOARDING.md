# KoadOS: Your First 15 Minutes

Welcome to **KoadOS**, a programmatic-first AI engineering partner. This guide will walk you through setting up your first session and establishing a baseline for your partnership.

## 1. Installation
Ensure you have **Rust** and **Python 3** installed.

```bash
git clone <your-fork-url> ~/.koad-os
cd ~/.koad-os
./koad-setup.sh --partner "YourName" --persona "Koad" --role "Partner" --langs "Rust,Python,Node.js"
```

## 2. Environment Setup
Add the binary to your path and set your API keys:

```bash
export KOAD_HOME="$HOME/.koad-os"
export PATH="$KOAD_HOME/bin:$PATH"
export GITHUB_PERSONAL_PAT="ghp_your_token"
export NOTION_TOKEN="secret_your_token"
```

## 3. "First Contact"
Once installed, run the boot sequence to initialize the persona:

```bash
koad boot
```

## 4. Registering Your First Project
Navigate to any repository you're working on and tell Koad to track it:

```bash
cd ~/projects/my-app
mkdir .koad
koad scan
```

## 5. Capturing Your First Fact
Koad learns from your actions. Try recording a finding:

```bash
koad remember fact "The API gateway uses us-east-1 for the staging environment." --tags "infra,staging"
```

---

**Next Steps:**
- Explore the **[Developer Guide](DEVELOPMENT.md)** to add your own Skills.
- Check the **[Architecture](ARCHITECTURE.md)** to understand the Spine and Memory.
