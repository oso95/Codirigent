//! Lucide icon integration for GPUI rendering.
//!
//! Provides the Lucide icon font and helper functions for rendering
//! icons as text elements in GPUI. Icons are rendered using the bundled
//! Lucide font with Unicode codepoints from the `lucide-icons` crate.

use lucide_icons::Icon;

/// Font family name for the Lucide icon font.
pub const LUCIDE_FONT_FAMILY: &str = "lucide";

/// Whether the Lucide font has been loaded into the text system.
#[cfg(feature = "gpui-full")]
static FONT_LOADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Load the Lucide icon font into GPUI's text system.
///
/// This should be called once during the first render pass.
/// Subsequent calls are no-ops.
#[cfg(feature = "gpui-full")]
pub fn ensure_font_loaded(window: &gpui::Window) {
    use std::sync::atomic::Ordering;
    if FONT_LOADED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let font_data: std::borrow::Cow<'static, [u8]> =
            std::borrow::Cow::Borrowed(lucide_icons::LUCIDE_FONT_BYTES);
        if let Err(e) = window.text_system().add_fonts(vec![font_data]) {
            tracing::warn!("Failed to load Lucide icon font: {}", e);
            FONT_LOADED.store(false, Ordering::SeqCst);
        } else {
            tracing::info!("Lucide icon font loaded successfully");
        }
    }
}

/// Get the Unicode character for a Lucide icon.
pub fn icon_char(icon: Icon) -> char {
    char::from(icon)
}

/// Get the Unicode character string for a Lucide icon.
pub fn icon_str(icon: Icon) -> String {
    String::from(icon_char(icon))
}

// Re-export commonly used icons for convenience.

/// Zap icon - used for logo/branding.
pub fn zap() -> String {
    icon_str(Icon::Zap)
}
/// FolderTree icon - file explorer.
pub fn folder_tree() -> String {
    icon_str(Icon::FolderTree)
}
/// GitBranch icon - worktrees/git.
pub fn git_branch() -> String {
    icon_str(Icon::GitBranch)
}
/// Settings icon - settings panel.
pub fn settings() -> String {
    icon_str(Icon::Settings)
}
/// Send icon - broadcast send.
pub fn send() -> String {
    icon_str(Icon::Send)
}
/// Columns icon - right panel toggle (split columns).
pub fn columns_3() -> String {
    icon_str(Icon::Columns3)
}
/// Cpu icon - token counter.
pub fn cpu() -> String {
    icon_str(Icon::Cpu)
}
/// ListTodo icon - task board.
pub fn list_todo() -> String {
    icon_str(Icon::ListTodo)
}
/// Terminal icon - session reference.
pub fn terminal() -> String {
    icon_str(Icon::Terminal)
}
/// Sparkles icon - AI/CLI indicator.
pub fn sparkles() -> String {
    icon_str(Icon::Sparkles)
}
/// AlertTriangle icon - input required warning.
pub fn alert_triangle() -> String {
    icon_str(Icon::TriangleAlert)
}
/// CheckCircle2 icon - completed/done.
pub fn check_circle() -> String {
    icon_str(Icon::CircleCheck)
}
/// X icon - close button.
pub fn x() -> String {
    icon_str(Icon::X)
}
/// ChevronRight icon - collapsed state.
pub fn chevron_right() -> String {
    icon_str(Icon::ChevronRight)
}
/// ChevronDown icon - expanded state.
pub fn chevron_down() -> String {
    icon_str(Icon::ChevronDown)
}
/// FileCode icon - source file.
pub fn file_code() -> String {
    icon_str(Icon::FileCode)
}
/// Search icon - search.
pub fn search() -> String {
    icon_str(Icon::Search)
}
/// LayoutGrid icon - grid layout.
pub fn layout_grid() -> String {
    icon_str(Icon::LayoutGrid)
}
/// Maximize2 icon - focus/single layout.
pub fn maximize() -> String {
    icon_str(Icon::Maximize2)
}
/// RefreshCw icon - refresh.
pub fn refresh() -> String {
    icon_str(Icon::RefreshCw)
}
/// GitCommit icon - git commit.
pub fn git_commit() -> String {
    icon_str(Icon::GitCommitHorizontal)
}
/// Play icon - working/running.
pub fn play() -> String {
    icon_str(Icon::Play)
}
/// Pause icon - paused.
pub fn pause() -> String {
    icon_str(Icon::Pause)
}
/// Square icon - stopped/idle.
pub fn square() -> String {
    icon_str(Icon::Square)
}
/// Clock icon - waiting.
pub fn clock() -> String {
    icon_str(Icon::Clock)
}
/// MoreHorizontal icon - overflow menu.
pub fn more_horizontal() -> String {
    icon_str(Icon::Ellipsis)
}
/// Plus icon - add/new.
pub fn plus() -> String {
    icon_str(Icon::Plus)
}
/// Wand2 icon - magic/AI.
pub fn wand() -> String {
    icon_str(Icon::Wand)
}
/// Lock icon - locked state.
pub fn lock() -> String {
    icon_str(Icon::Lock)
}
/// ArrowRight icon - arrow indicator.
pub fn arrow_right() -> String {
    icon_str(Icon::ArrowRight)
}
/// Command icon - keyboard shortcut.
pub fn command() -> String {
    icon_str(Icon::Command)
}
/// Pencil icon - rename/edit.
pub fn pencil() -> String {
    icon_str(Icon::Pencil)
}
/// Users icon - group/team.
pub fn users() -> String {
    icon_str(Icon::Users)
}
/// UserMinus icon - remove from group.
pub fn user_minus() -> String {
    icon_str(Icon::UserMinus)
}
/// XCircle icon - close/remove.
pub fn x_circle() -> String {
    icon_str(Icon::CircleX)
}
/// Check icon - confirm/apply.
pub fn check() -> String {
    icon_str(Icon::Check)
}
/// GitFork icon - existing branch.
pub fn git_fork() -> String {
    icon_str(Icon::GitFork)
}
/// Radio icon - broadcast.
pub fn radio() -> String {
    icon_str(Icon::Radio)
}
/// CirclePlus icon - add new (circle variant).
pub fn circle_plus() -> String {
    icon_str(Icon::CirclePlus)
}
/// PlusCircle icon - create task.
pub fn clipboard_plus() -> String {
    icon_str(Icon::ClipboardPlus)
}
/// Folder icon - closed directory.
pub fn folder() -> String {
    icon_str(Icon::Folder)
}
/// FolderOpen icon - expanded directory.
pub fn folder_open() -> String {
    icon_str(Icon::FolderOpen)
}
/// File icon - generic file.
pub fn file() -> String {
    icon_str(Icon::File)
}
/// Eye icon - visible/show.
pub fn eye() -> String {
    icon_str(Icon::Eye)
}
/// EyeOff icon - hidden/hide.
pub fn eye_off() -> String {
    icon_str(Icon::EyeOff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_char_returns_valid_char() {
        let ch = icon_char(Icon::Zap);
        assert!(ch as u32 > 0);
    }

    #[test]
    fn test_icon_str_not_empty() {
        let s = icon_str(Icon::Settings);
        assert!(!s.is_empty());
    }

    #[test]
    fn test_all_icon_helpers_return_non_empty() {
        assert!(!zap().is_empty());
        assert!(!folder_tree().is_empty());
        assert!(!git_branch().is_empty());
        assert!(!settings().is_empty());
        assert!(!send().is_empty());
        assert!(!columns_3().is_empty());
        assert!(!cpu().is_empty());
        assert!(!list_todo().is_empty());
        assert!(!terminal().is_empty());
        assert!(!sparkles().is_empty());
        assert!(!alert_triangle().is_empty());
        assert!(!check_circle().is_empty());
        assert!(!x().is_empty());
        assert!(!chevron_right().is_empty());
        assert!(!chevron_down().is_empty());
        assert!(!file_code().is_empty());
        assert!(!search().is_empty());
        assert!(!layout_grid().is_empty());
        assert!(!maximize().is_empty());
        assert!(!refresh().is_empty());
        assert!(!git_commit().is_empty());
        assert!(!play().is_empty());
        assert!(!pause().is_empty());
        assert!(!square().is_empty());
        assert!(!clock().is_empty());
        assert!(!more_horizontal().is_empty());
        assert!(!plus().is_empty());
        assert!(!wand().is_empty());
        assert!(!lock().is_empty());
        assert!(!arrow_right().is_empty());
        assert!(!command().is_empty());
        assert!(!pencil().is_empty());
        assert!(!users().is_empty());
        assert!(!user_minus().is_empty());
        assert!(!x_circle().is_empty());
        assert!(!check().is_empty());
        assert!(!git_fork().is_empty());
        assert!(!radio().is_empty());
        assert!(!circle_plus().is_empty());
        assert!(!clipboard_plus().is_empty());
        assert!(!folder().is_empty());
        assert!(!folder_open().is_empty());
        assert!(!file().is_empty());
        assert!(!eye().is_empty());
        assert!(!eye_off().is_empty());
    }

    #[test]
    fn test_font_family_constant() {
        assert_eq!(LUCIDE_FONT_FAMILY, "lucide");
    }
}
