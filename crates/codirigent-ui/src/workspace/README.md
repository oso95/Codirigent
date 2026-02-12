# Workspace Module

The workspace module manages the main application window with grid layout, session panes, and UI controls.

## Architecture

The workspace is split into two main components:

- **`core.rs`** - Core workspace logic (layout, sessions, focus management)
- **`gpui.rs`** - GPUI view implementation and state management

The rendering logic is further split into focused component modules for maintainability.

## Rendering Modules

The workspace rendering is organized into specialized modules:

### Core Coordinator
- **`render.rs`** (2,461 lines) - Main rendering coordinator
  - Terminal content rendering
  - Drawer panels (sessions, files, worktrees)
  - Session menus and inline UI
  - Module coordination

### Component Renderers
- **`grid_render.rs`** (729 lines) - Grid and split layouts
  - Traditional NxM grid layout
  - Split tree (binary tree) layout
  - Session cells with terminals
  - Empty cell placeholders

- **`task_board_render.rs`** (1,334 lines) - Task management UI
  - Right sidebar task board
  - Task creation and editing modals
  - Task cards and status sections
  - Priority and status mapping

- **`modal_render.rs`** (963 lines) - Modal dialogs
  - Custom layout builder modal
  - Session action modal (rename, group assign)
  - Modal overlay and interactions

- **`icon_rail_render.rs`** (174 lines) - Left sidebar
  - Icon rail navigation
  - Icon click handling
  - Rail layout and styling

- **`top_bar_render.rs`** (172 lines) - Top bar UI
  - Session tabs
  - Layout controls
  - Window controls integration

### Utilities
- **`icon_utils.rs`** (191 lines) - Icon rendering helpers
  - `centered_lucide_icon()` - Centered icon wrapper
  - `aligned_icon_label_row()` - Icon + label rows
  - Consistent icon alignment utilities

## Key Types

### Core Types
- **`Workspace`** - Core workspace state and logic
  - Layout management (grid, split tree, single)
  - Session collection
  - Focus tracking
  - Bounds calculation

- **`CellInfo`** - Information about grid cells
  - Session assignment
  - Cell bounds
  - Visual state

### GPUI Types
- **`WorkspaceView`** - Main GPUI view
  - Rendering implementation
  - Event handling
  - UI state management
  - Terminal views

## Layout System

The workspace supports three layout modes:

1. **Grid Layout** (1x1 to 3x3)
   - Traditional grid of session panes
   - Fixed or flexible cell sizing
   - Rendered by `grid_render.rs`

2. **Split Tree Layout**
   - Binary tree of horizontal/vertical splits
   - Recursive pane subdivision
   - Rendered by `grid_render.rs`

3. **Single Layout**
   - Focused single session view
   - Quick switching between sessions
   - Temporary overlay mode

## Session Management

Sessions represent terminal instances with associated state:

- **Session Creation** - Create new sessions in grid cells
- **Session Focus** - Track and switch focused session
- **Session Grouping** - Organize sessions by color-coded groups
- **Session Persistence** - Sessions persist across layout changes

## UI Components

### Top Bar
- Session tabs with labels and status
- Layout switcher (grid, split, single)
- Window controls (minimize, maximize, close)

### Icon Rail (Left Sidebar)
- Navigation icons
- Layout mode selector
- Drawer toggle

### Drawer Panels (Left)
- **Sessions** - List of all sessions with groups
- **Files** - Git changes and file tree
- **Worktrees** - Git worktree management

### Task Board (Right Sidebar)
- Task queue by status
- Task creation and editing
- Auto-assignment configuration
- Task actions (assign, review, complete)

## Event Handling

The workspace processes events through dedicated handlers:

- **UI Events** - Button clicks, modal actions, task board events
- **Top Bar Events** - Session tab clicks, layout changes
- **Icon Rail Events** - Navigation, drawer toggle
- **Keyboard Shortcuts** - Session switching, layout changes
- **Terminal Events** - Mouse/keyboard input, scrolling

## Testing

The workspace module includes comprehensive tests:

- **Layout Tests** - Grid dimensions, cell bounds
- **Session Tests** - Add/remove/focus sessions
- **Focus Tests** - Session number navigation
- **Bounds Tests** - Cell and sidebar calculations
- **Theme Tests** - Theme application and updates

Run tests with:
```bash
cargo test -p codirigent-ui --lib workspace::
```

## Rendering Pattern

All rendering modules use a consistent pattern:

```rust
// In each *_render.rs module
impl WorkspaceView {
    pub(super) fn render_component(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        // Component rendering logic
    }
}
```

This keeps methods accessible via `self` without changing the public API.

## Module Dependencies

```
workspace/
├── core.rs              # Core logic (no GPUI dependencies)
├── gpui.rs              # GPUI view (depends on core)
├── render.rs            # Main coordinator
│   ├── Uses: grid_render, icon_rail_render, task_board_render
│   ├── Uses: top_bar_render, modal_render
│   └── Uses: icon_utils
├── grid_render.rs       # Grid/split layouts
│   └── Uses: icon_utils
├── task_board_render.rs # Task board UI
│   └── Uses: icon_utils
├── icon_rail_render.rs  # Left sidebar
│   └── Uses: icon_utils
├── top_bar_render.rs    # Top bar
│   └── Uses: icon_utils
├── modal_render.rs      # Modal dialogs
│   └── Uses: icon_utils
└── icon_utils.rs        # Shared utilities
```

## Performance Considerations

- **Incremental Rendering** - Only changed components re-render
- **Terminal Throttling** - Terminal resizes throttled to ~10/sec
- **Lazy Font Detection** - Monospace fonts detected once on first render
- **Efficient Bounds** - Cell bounds calculated once per layout change

## Future Improvements

Potential enhancements for the workspace module:

1. **Layout Persistence** - Save/restore layout across sessions
2. **Custom Layouts** - User-defined split configurations
3. **Session Templates** - Pre-configured session setups
4. **Enhanced Task Board** - Drag-drop task reordering
5. **Multi-Window Support** - Multiple workspace windows
