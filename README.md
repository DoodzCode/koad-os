# KoadOS: The AI Software Engineering Partner

**KoadOS** is an open-source, programmatic-first framework for building a long-term, context-aware partnership between a developer and an AI agent. 

It acts as an **Operating System for your Agent**, providing a persistent memory (SQLite), a background cognitive booster (Rust Daemon), and an extensible skill system (Python/JS).

## 🚀 Key Features

- **Contextual Boot**: Instantly inject identity, project state, and relevant facts into your AI agent's context window.
- **Surgical Search**: Retrieve only the most relevant knowledge to minimize token usage and maximize reasoning accuracy.
- **Background Booster**: A Rust-based daemon tracks file changes and summarizes local activity in real-time.
- **Standardized Skills**: Extend Koad's capabilities with simple scripts that can interface with GCloud, Notion, Airtable, and more.

## 🛠️ Getting Started

To install KoadOS and initialize your first partner, follow our **[Onboarding Guide](docs/ONBOARDING.md)**.

```bash
git clone https://github.com/DoodzCode/koad-os.git ~/.koad-os
cd ~/.koad-os
./koad-setup.sh --partner "YourName" --persona "Koad" --role "Partner" --langs "Rust,Python,Node.js"
```

## 📖 Documentation

- **[Onboarding](docs/ONBOARDING.md)**: Your first 15 minutes.
- **[Developer Guide](docs/DEVELOPMENT.md)**: How to write skills and contribute to the core.
- **[Architecture](docs/ARCHITECTURE.md)**: Deep dive into the Spine, Memory, and Drivers.
- **[Technical Spec](SPEC.md)**: Full CLI reference and database schema.

## 🤝 Contributing

We welcome contributions! Whether you're adding a new global skill or improving the Rust core, please check our **[Developer Guide](docs/DEVELOPMENT.md)** to get started.

## ⚖️ License

MIT License. See [LICENSE.md](LICENSE.md) for details. (Drafting soon!)
