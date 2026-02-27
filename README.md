# Geisterhand

Linux screen automation tool with HTTP API and CLI. Automate any Linux app — click buttons, type text, navigate menus, read accessibility trees, and capture screenshots. Built on AT-SPI2 and designed for GNOME (X11 and Wayland).

## Install

```bash
cargo install geisterhand
```

<details>
<summary>Other install methods</summary>

**From GitHub:**
```bash
cargo install --git https://github.com/Geisterhand-io/linux.git
```

**Build from source:**
```bash
git clone https://github.com/Geisterhand-io/linux.git && cd linux
cargo build --release
sudo install -m 755 target/release/geisterhand /usr/local/bin/
```

**Arch Linux (AUR):**
```bash
yay -S geisterhand
```
</details>

## Prerequisites

Geisterhand uses AT-SPI2 for accessibility and either XTest (X11) or uinput (Wayland) for input injection. Most GNOME desktops have these ready out of the box.

```bash
# Check what's available
geisterhand check gnome-calculator

# Enable accessibility if needed (GNOME)
gsettings set org.gnome.desktop.interface toolkit-accessibility true

# For Wayland uinput access (optional — XTest works on XWayland)
sudo usermod -aG input $USER  # then re-login
```

## Quick Start

**1. Start a server scoped to an app:**

```bash
geisterhand run gnome-calculator
# {"port":7676,"host":"127.0.0.1","version":"0.1.0"}
```

**2. Automate:**

```bash
# See what's on screen
curl http://127.0.0.1:7676/accessibility/tree?format=compact

# Click a button
curl -X POST http://127.0.0.1:7676/click/element \
  -H "Content-Type: application/json" \
  -d '{"title": "7", "role": "push button"}'

# Type text
curl -X POST http://127.0.0.1:7676/type \
  -H "Content-Type: application/json" \
  -d '{"text": "Hello World"}'

# Take a screenshot
curl http://127.0.0.1:7676/screenshot --output screen.png
```

## `geisterhand run`

The primary way to use Geisterhand. Launches an app and starts an HTTP server scoped to it:

```bash
geisterhand run gnome-calculator           # by app name or command
geisterhand run gnome-text-editor          # launch any app
geisterhand run gnome-calculator --port 8080  # pin a specific port
```

The server auto-selects a free port (starting from 7676), scopes all requests to the app's PID, and exits when the app quits. Connection details are printed as JSON on stdout.

## HTTP API

All endpoints accept and return JSON with `snake_case` field names.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/status` | System info, permissions, frontmost app |
| GET | `/screenshot` | Capture screen (`?format=png\|jpeg`) |
| POST | `/click` | Click at coordinates |
| POST | `/click/element` | Click element by title/role/label |
| POST | `/type` | Type text |
| POST | `/key` | Press key with modifiers |
| POST | `/scroll` | Scroll at position |
| POST | `/wait` | Wait for element to appear/disappear/become enabled |
| GET | `/accessibility/tree` | Get UI element hierarchy (`?format=compact`) |
| GET | `/accessibility/elements` | Find elements by role/title/label |
| GET | `/accessibility/focused` | Get focused element |
| POST | `/accessibility/action` | Perform action on element (press, setValue, focus, ...) |
| GET | `/menu` | Get application menu structure |
| POST | `/menu` | Trigger menu item |

## CLI

```bash
geisterhand server                          # start HTTP server
geisterhand server --port 8080              # custom port
geisterhand run gnome-calculator            # launch app + scoped server
geisterhand screenshot -o screen.png        # capture screen
geisterhand click -x 100 -y 200            # click at coordinates
geisterhand click --title "OK" --role "push button"  # click element
geisterhand type "Hello World"              # type text
geisterhand key s --modifiers ctrl          # press Ctrl+S
geisterhand scroll --dy -3                  # scroll up
geisterhand status                          # query running server
geisterhand check gnome-calculator          # check accessibility support
geisterhand mcp                             # run as MCP server (stdio)
```

## Using with Claude

Add Geisterhand as an MCP server for Claude Code or Claude Desktop:

```bash
# Claude Code
claude mcp add geisterhand -- geisterhand mcp
```

<details>
<summary>Claude Desktop</summary>

Add to `~/.config/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "geisterhand": {
      "command": "geisterhand",
      "args": ["mcp"]
    }
  }
}
```
</details>

The MCP server exposes 12 tools: `screenshot`, `click`, `click_element`, `type_text`, `key_press`, `scroll`, `get_tree`, `find_elements`, `get_focused`, `perform_action`, `wait`, `status`.

## Systemd Service

To run the server persistently:

```bash
# Install the user service
cp geisterhand.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now geisterhand
```

## Cross-Platform

Geisterhand has identical HTTP APIs on macOS and Linux. The same client code works on both — only role names differ:

| macOS (AXRole) | Linux (AT-SPI2) |
|----------------|-----------------|
| `AXButton` | `push button` |
| `AXTextField` | `text` |
| `AXStaticText` | `label` |
| `AXWindow` | `frame` |

## Requirements

- Linux with GNOME (X11 or Wayland)
- AT-SPI2 enabled (most GNOME installs have this)
- Rust 1.75+ (for building)

## License

MIT
