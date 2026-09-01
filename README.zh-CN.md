<p align="center">
  <img src="assets/icons/logo-primary-dark.svg" alt="Codirigent Logo" width="128" height="128" />
</p>

<h1 align="center">Codirigent</h1>

<p align="center">
  <strong>一个可并行运行多个 AI Coding CLI 的终端工作空间</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/状态-alpha-orange" alt="项目状态" />
  <img src="https://img.shields.io/badge/版本-0.1.0-blue" alt="版本" />
  <img src="https://img.shields.io/badge/许可证-GPL--3.0-blue" alt="许可证" />
  <img src="https://img.shields.io/badge/rust-1.75%2B-red" alt="Rust 版本" />
</p>

<p align="center">
  <a href="https://codirigent.dev">官网</a> ·
  <a href="https://github.com/oso95/Codirigent/releases/latest">下载</a> ·
  <a href="https://github.com/oso95/Codirigent/issues">报告问题</a> ·
  <a href="./README.md">English</a> ·
  <a href="./README.zh-TW.md">繁體中文</a>
</p>

---

https://github.com/user-attachments/assets/51b821fd-dfc3-40f0-b1f3-e8727045f474

---

如果你同时在多个项目中运行 Claude Code、Codex 或 Gemini，你一定知道那种痛苦：打开终端、在仓库之间 `cd` 来回切换、摆放窗口，还要记住哪个 agent 正在做什么。

Codirigent 是一个为这种工作流打造的 Tmux 风格工作空间。只要打开一次，你的会话就会保持在上次离开时的状态，目录正确、布局正确、agent 也正确。

## 功能特性

**多个会话，一个视图** — 并排运行 Claude Code、Codex 和 Gemini。每个会话都会显示实时状态指示器：

| 状态 | | 含义 |
|--------|---|---------|
| 空闲 | ![gray](https://img.shields.io/badge/●-gray) | Shell 空闲，没有 agent 活动 |
| 工作中 | ![amber](https://img.shields.io/badge/●-f59e0b) | Agent 正在生成回复 |
| 需要关注 | ![rose](https://img.shields.io/badge/●-f43f5e) | Agent 正在等待用户输入或授权 |
| 就绪 | ![green](https://img.shields.io/badge/●-22c55e) | Agent 已完成，回复正在未聚焦的会话中等待 |

---

**自定义布局** — 以任意网格方式排列会话并保存。拖放会话标题即可随时重新安排位置。

---

**同步文件树** — 文件浏览器始终反映当前聚焦的会话，因此你总能知道自己当前位于哪里。

---

**Git worktree 支持** — 在隔离分支上同时运行多个 agent，互不冲突。

---

**会话恢复** — Codirigent 会自动检测并恢复之前的 Claude Code 和 Codex 会话，让你可以从上次中断的地方继续。

---

**智能剪贴板** — 可将文本、文件或图片粘贴到任意会话中。文件路径会自动转换为目标 CLI 可直接使用的 shell 友好格式。

## 下载

> **早期 Alpha 版本** — 预计还会有不少粗糙之处。[欢迎反馈。](https://github.com/oso95/Codirigent/issues)

### Windows

从[最新版本](https://github.com/oso95/Codirigent/releases/latest)下载 `.msi` 安装程序。

> **SmartScreen 警告：** 由于应用尚未进行代码签名，Windows 可能会显示“Windows 已保护你的电脑”。点击 **更多信息 → 仍要运行** 继续。

### macOS

从[最新版本](https://github.com/oso95/Codirigent/releases/latest)下载 `.dmg`。

## Hook 设置（推荐）

Codirigent 使用轻量级 hook 来实时跟踪 agent 状态，显示每个会话当前是 Working、Needs Attention 还是 Response Ready。如果 hook 不可用，Codirigent 会回退到 reader/detector 路径，但精度会稍差一些。

**支持的 CLI 会在首次启动时自动安装 Hooks。** Codirigent 会将它的 `codirigent-hook` 二进制注册到各个 CLI 的配置中：

| CLI | 配置文件 | 自动安装 |
|-----|-------------|----------------|
| Claude Code | `~/.claude/settings.json` | 是 |
| Codex CLI | `~/.codex/config.toml` | 是 |
| Gemini CLI | `~/.gemini/settings.json` | 是 |

如需确认 hook 已安装，请检查你的 CLI 配置文件中是否出现了 `codirigent-hook`。如果你移动或重新安装了 Codirigent，只需重新启动一次，即可用更新后的二进制路径重新注册 hooks。

## 从源码构建

**前置要求：** Rust 1.75+、Windows 或 macOS

```bash
git clone https://github.com/oso95/Codirigent.git
cd Codirigent
cargo install --path . --all-features
cargo install --path crates/codirigent-hook
```

这会将 `codirigent` 和 `codirigent-hook` 一起安装到 `~/.cargo/bin/`。hook 二进制是实时跟踪 agent 状态所必需的组件（见[Hook 设置](#hook-设置推荐)）。

如需在不安装的情况下运行：

```bash
cargo run --all-features
```

> Linux 支持尚未完成。

## 开发

```bash
cargo test --all --all-targets        # 运行测试
cargo fmt --all                       # 格式化
cargo clippy --all -- -D warnings     # lint 检查
```

## 贡献

进行重大更改前请先开 issue 讨论。欢迎提交 PR。

## 许可证

GPL-3.0 — 详见 [LICENSE](LICENSE)。
