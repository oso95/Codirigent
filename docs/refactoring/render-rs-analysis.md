# render.rs Component Analysis

## Current Structure (5,836 lines)

### Overview
The render.rs file contains all GPUI rendering logic for the WorkspaceView, including:
- Workspace grid layout and split tree rendering
- Icon rail (left sidebar)
- Task board (right panel)
- Top bar with session tabs
- Various modals (rename, group assign, task creation)
- Terminal content rendering
- Drawer panels (sessions, files, worktrees)

### Component Groups

#### 1. Utility Functions (~150 lines)
- `centered_lucide_icon()` - Fixed square icon wrapper
- `centered_lucide_icon_with_offset()` - Icon with vertical offset
- `aligned_icon_label_row()` - Icon + label row layout
- `aligned_icon_label_row_with_offset()` - Icon + label with offset
- `parse_group_color()` - Color string parser
- `render_logo_small()` - Small logo rendering

#### 2. Task Board (~1,200 lines, estimated)
- `core_task_to_ui_item()` - Convert core Task to UI TaskItem
- `build_priority_button()` - Priority selection button
- `render_task_card()` - Individual task card rendering
- Task board panel layout with sections (Queued, In Progress, Review, Done)
- Task creation modal
- Priority and status mapping

#### 3. Grid & Split Layout (~1,500 lines, estimated)
- `render_grid_layout()` - Main grid rendering (1-9 cells)
- `render_split_tree_layout()` - Split tree rendering
- `render_split_node()` - Recursive split node rendering
- `render_split_empty_slot()` - Empty slot placeholders
- `render_session_cell_with_terminal()` - Session cell content
- `render_grid_builder_content()` - Grid builder UI
- `render_split_builder_content()` - Split builder UI
- `render_grid_preview()` - Grid preview during build
- `render_split_preview_node()` - Split preview during build

#### 4. Terminal Rendering (~400 lines, estimated)
- `render_terminal_content()` - Terminal text and cursor
- `render_terminal_header_inline()` - Inline terminal header
- `render_empty_cell_inline()` - Empty cell placeholders
- `render_empty_cell_inline_with_colors()` - Custom colored empty cells

#### 5. Drawer Panels (~900 lines, estimated)
- `render_drawer_sessions_content()` - Sessions drawer
- `render_drawer_worktrees_content()` - Worktrees drawer
- `render_drawer_files_content()` - Files/changes drawer
- `render_session_row()` - Session list item
- `render_session_group_header()` - Group header in sessions
- `render_file_tree_row()` - File tree item
- `change_kind_display()` - Git change type display

#### 6. Menu Items (~200 lines, estimated)
- `render_menu_item()` - Generic menu item rendering
- Session menu actions
- Context menu rendering

#### 7. Top Bar & Icon Rail (~500 lines, estimated via Render trait impl)
- Top bar rendering with session tabs
- Icon rail (left sidebar) with navigation icons
- Status indicators
- Workspace controls

#### 8. Modals (~800 lines, estimated)
- Task creation modal (large section at end)
- Rename modal
- Group assignment modal
- Action confirmation modals

### Function Count
- Total functions: 50+ (estimate)
- `render_*` functions: 22
- Helper functions: 28+
- Public API: Main `Render` trait implementation

### Dependencies Between Components

```
Render trait (main entry)
  ├─> Grid Layout
  │     ├─> Session Cells
  │     │     └─> Terminal Content
  │     └─> Empty Cells
  ├─> Icon Rail (left sidebar)
  ├─> Top Bar
  │     └─> Session Tabs
  ├─> Task Board (right panel)
  │     ├─> Task Cards
  │     └─> Task Creation Modal
  └─> Drawer Panels
        ├─> Sessions Drawer
        ├─> Files Drawer
        └─> Worktrees Drawer
```

### Shared Utilities
The following utilities are used across multiple components:
- `centered_lucide_icon()` - Used by: task board, drawer, top bar, modals
- `aligned_icon_label_row()` - Used by: drawer, menus, task board

### Extraction Strategy

Based on this analysis, the extraction order should be:

1. **icon_utils.rs** - Extract shared icon utilities first (no dependencies)
2. **task_board_render.rs** - Extract task board (uses icon utils)
3. **icon_rail_render.rs** - Extract icon rail (minimal dependencies)
4. **top_bar_render.rs** - Extract top bar (minimal dependencies)
5. **modal_render.rs** - Extract modals (may use utilities)
6. **grid_render.rs** - Extract grid layout last (coordinates everything)
7. **drawer_render.rs** - Extract drawer panels (optional, could stay in render.rs if needed)

### Line Count Estimates After Split

- `render.rs` (coordinator): ~200 lines
- `icon_utils.rs`: ~150 lines
- `task_board_render.rs`: ~1,200 lines
- `grid_render.rs`: ~1,500 lines
- `drawer_render.rs`: ~900 lines
- `terminal_render.rs`: ~400 lines
- `icon_rail_render.rs`: ~300 lines
- `top_bar_render.rs`: ~300 lines
- `modal_render.rs`: ~800 lines
- `menu_render.rs`: ~200 lines

**Total:** ~5,950 lines (accounts for some module boilerplate)

### Critical Notes

1. All extracted modules will use `impl WorkspaceView` pattern to maintain method access
2. No public API changes required - all methods remain accessible via `self`
3. Module imports will be added to `mod.rs`
4. The main `Render` trait implementation will remain in `render.rs` and call methods from extracted modules
