# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Geisterhand is a Linux screen automation tool that provides both an HTTP API and CLI for controlling mouse, keyboard, reading accessibility trees, and capturing screenshots. It uses AT-SPI2 for accessibility, XTest/uinput for input injection, and xdg-desktop-portal/X11 for screenshots.

## Build Commands

```bash
# Build
cargo build

# Build for release
cargo build --release

# Run the CLI tool
cargo run -- server
cargo run -- mcp

# Check for warnings
cargo clippy
```

## Architecture

Single binary with library core (`src/lib.rs`) and CLI entry (`src/main.rs`).

### Core Modules

- **server/http.rs**: Axum HTTP server on port 7676. `build_router()` wires all routes. `AppState` holds optional `TargetApp` for scoped mode.
- **server/routes/**: Individual route handlers — each file handles one endpoint group (status, screenshot, click, type_text, key, scroll, wait, accessibility, menu).
- **accessibility/service.rs**: AT-SPI2 service via `atspi` crate. Tree traversal, element search by role/title/label, actions (press, setValue, focus). Uses `AccessibilityConnection::new()` for the dedicated a11y D-Bus bus. Key functions: `get_compact_tree()`, `find_elements()`, `perform_action()`, `get_focused_element()`, `get_frontmost_app()`.
- **accessibility/menu.rs**: Menu bar traversal and triggering via AT-SPI2.
- **input/mod.rs**: `InputBackend` trait with `get_backend()` auto-selection (XTest on X11, uinput on Wayland).
- **input/xtest.rs**: X11 XTest extension backend via `x11rb`. Uses `xproto::*_EVENT` constants with `fake_input()`.
- **input/uinput.rs**: Virtual device backend via `/dev/uinput` for Wayland. Needs input group membership.
- **input/keycode_map.rs**: Linux evdev keycode mappings (key names to `KEY_*` codes).
- **screen/mod.rs**: Screenshot orchestration. Tries X11 capture on X11, portal on Wayland, with fallbacks.
- **screen/x11.rs**: X11 `GetImage` capture with BGRA-to-RGBA conversion.
- **screen/portal.rs**: xdg-desktop-portal Screenshot D-Bus API with fallback to gnome-screenshot/grim CLI tools.
- **mcp/mod.rs**: MCP (Model Context Protocol) server — JSON-RPC 2.0 over stdio. Exposes 12 tools matching the HTTP endpoints.
- **models/api.rs**: All HTTP request/response types (serde Serialize/Deserialize).
- **models/accessibility.rs**: `ElementPath`, `UIElementInfo`, `ElementQuery`, `AccessibilityAction`, `CompactElement`.
- **platform/display.rs**: Display server detection (Wayland vs X11), screen size.
- **platform/permissions.rs**: Runtime checks for AT-SPI2, uinput, screen capture availability.
- **platform/process.rs**: Process lookup by PID, frontmost app detection.

### CLI Subcommands (main.rs)

`server`, `run`, `screenshot`, `click`, `type`, `key`, `scroll`, `status`, `check`, `mcp`

## HTTP API Endpoints

All endpoints run on `127.0.0.1:7676`:

- `GET /status` — System info and permission status
- `GET /screenshot` — Capture screen (`?format=png|jpeg`)
- `POST /click` — Click at coordinates (`{x, y, button, click_count, modifiers}`)
- `POST /click/element` — Click element by title/role/label (`{title, role, pid, use_accessibility_action}`)
- `POST /type` — Type text (`{text, delay_ms, pid, path, role, title}`)
- `POST /key` — Press key with modifiers (`{key, modifiers, pid, path}`)
- `POST /scroll` — Scroll at position (`{x, y, delta_x, delta_y, pid}`)
- `POST /wait` — Wait for element state (`{role, title, condition, timeout_ms}`)
- `GET /accessibility/tree` — UI element hierarchy (`?pid=&max_depth=&format=compact`)
- `GET /accessibility/elements` — Find elements (`?role=&title=&title_contains=&pid=`)
- `GET /accessibility/focused` — Focused element (`?pid=`)
- `POST /accessibility/action` — Perform action (`{pid, path, action, value}`)
- `GET /menu` — Menu structure (`?pid=`)
- `POST /menu` — Trigger menu item (`{pid, path}`)

## Key Implementation Details

- AT-SPI2 connects to the dedicated a11y bus via `AccessibilityConnection::new()`, not the session bus
- AT-SPI2 registry root is at `/org/a11y/atspi/accessible/root`
- Build interface proxies (ActionProxy, ComponentProxy) via `::builder(conn)` — do NOT use `ProxyExt::proxies()` (fails on GTK4)
- GTK4 apps: `nactions` D-Bus property may not work; enumerate actions with `get_name(i)` in a loop
- PID resolution: `DBusProxy::get_connection_unix_process_id()` on the a11y bus
- XTest: `fake_input` takes 8 args including `deviceid: u8` (pass 0 for core device)
- JPEG encoding requires RGBA-to-RGB conversion (JPEG has no alpha channel)
- JSON uses `snake_case` field names throughout

## Cross-Platform Compatibility

This is the Linux port of the macOS Geisterhand. Same HTTP API, same JSON shapes. Key differences:
- Roles: AT-SPI2 names (`push button`) instead of AX names (`AXButton`)
- Modifiers: `super` instead of `cmd` (both accepted)
- `desktop_file` instead of `bundle_identifier`
- Permissions: `at_spi2_available`, `uinput_access`, `screen_capture_available`
