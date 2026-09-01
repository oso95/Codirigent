<p align="center">
  <img src="assets/icons/logo-primary-dark.svg" alt="Codirigent Logo" width="128" height="128" />
</p>

<h1 align="center">Codirigent</h1>

<p align="center">
  <strong>在同一個工作空間中並行運行多個 AI Coding CLI 的終端工作空間</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/狀態-alpha-orange" alt="專案狀態" />
  <img src="https://img.shields.io/badge/版本-0.1.0-blue" alt="版本" />
  <img src="https://img.shields.io/badge/授權-GPL--3.0-blue" alt="授權" />
  <img src="https://img.shields.io/badge/rust-1.75%2B-red" alt="Rust 版本" />
</p>

<p align="center">
  <a href="https://codirigent.dev">官方網站</a> ·
  <a href="https://github.com/oso95/Codirigent/releases/latest">下載</a> ·
  <a href="https://github.com/oso95/Codirigent/issues">回報問題</a> ·
  <a href="./README.md">English</a> ·
  <a href="./README.zh-CN.md">简体中文</a>
</p>

---

https://github.com/user-attachments/assets/51b821fd-dfc3-40f0-b1f3-e8727045f474

---

如果你同時在多個專案中使用 Claude Code、Codex 或 Gemini，你一定懂那種痛苦：不斷開啟終端、`cd` 切換目錄、排列視窗，還要搞清楚哪個 AI 代理在做什麼。

Codirigent 是一個專為這種工作流程打造的 Tmux 風格工作空間。啟動一次，你的所有 session 就已在原本的位置等你——正確的目錄、正確的版面、正確的代理。

## 功能特色

**多 session，單一視圖** — 並排執行 Claude Code、Codex 與 Gemini。每個 session 都有即時狀態指示器：

| 狀態 | | 說明 |
|------|---|------|
| 閒置 | ![gray](https://img.shields.io/badge/●-gray) | Shell 閒置，無代理活動 |
| 工作中 | ![amber](https://img.shields.io/badge/●-f59e0b) | 代理正在生成回應 |
| 需要注意 | ![rose](https://img.shields.io/badge/●-f43f5e) | 代理正在等待使用者輸入或授權 |
| 已就緒 | ![green](https://img.shields.io/badge/●-22c55e) | 代理已完成，回應在未聚焦的 session 中等待 |

---

**自訂版面** — 以任意網格配置排列 session 並儲存。拖放 session 標題即可即時調整位置。

---

**同步檔案樹** — 檔案瀏覽器會隨時反映目前聚焦的 session，讓你隨時掌握位置。

---

**Git Worktree 支援** — 在獨立分支上同時執行多個代理，互不衝突。

---

**Session 恢復** — Codirigent 會自動偵測並恢復先前的 Claude Code 與 Codex session，讓你從中斷處無縫繼續。

---

**智慧剪貼簿** — 將文字、檔案或圖片貼入任意 session。檔案路徑會自動轉換為目標 CLI 適用的 shell 格式。

## 下載

> **早期 Alpha 版本** — 可能有些粗糙之處。[歡迎提供意見回饋。](https://github.com/oso95/Codirigent/issues)

### Windows

從[最新版本](https://github.com/oso95/Codirigent/releases/latest)下載 `.msi` 安裝程式。

> **SmartScreen 警告：** 由於應用程式尚未進行程式碼簽章，Windows 可能會顯示「Windows 已保護您的電腦」。請點擊 **更多資訊 → 仍要執行** 繼續安裝。

### macOS

從[最新版本](https://github.com/oso95/Codirigent/releases/latest)下載 `.dmg` 檔案。

## Hook 設定（建議）

Codirigent 使用輕量 hook 即時追蹤代理狀態——顯示每個 session 是否正在工作、需要注意，或已有回應等待確認。若 hook 無法使用，Codirigent 會退回至讀取器/偵測器模式，精確度較低。

**Hook 在首次啟動時會自動安裝**，支援的 CLI 皆可自動註冊。Codirigent 會將其 `codirigent-hook` 執行檔註冊至各 CLI 的設定中：

| CLI | 設定檔 | 自動安裝 |
|-----|--------|---------|
| Claude Code | `~/.claude/settings.json` | 是 |
| Codex CLI | `~/.codex/config.toml` | 是 |
| Gemini CLI | `~/.gemini/settings.json` | 是 |

若要確認 hook 已安裝，請檢查各 CLI 設定檔中是否出現 `codirigent-hook`。若你移動或重新安裝 Codirigent，只需重新啟動一次，即可以更新後的執行檔路徑重新註冊 hook。

## 從原始碼建置

**前置需求：** Rust 1.75+、Windows 或 macOS

```bash
git clone https://github.com/oso95/Codirigent.git
cd Codirigent
cargo install --path . --all-features
cargo install --path crates/codirigent-hook
```

此指令會將 `codirigent` 與 `codirigent-hook` 安裝至 `~/.cargo/bin/`。hook 執行檔是即時代理狀態追蹤的必要元件（請參閱 [Hook 設定](#hook-設定建議)）。

若要在不安裝的情況下執行：

```bash
cargo run --all-features
```

> Linux 支援尚未完成。

## 開發

```bash
cargo test --all --all-targets        # 執行測試
cargo fmt --all                       # 格式化
cargo clippy --all -- -D warnings     # 靜態分析
```

## 貢獻

重大變更請先開 issue 討論。歡迎提交 PR。

## 授權

GPL-3.0 — 詳見 [LICENSE](LICENSE)。
