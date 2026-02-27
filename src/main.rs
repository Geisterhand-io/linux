use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use geisterhand::server::http;

const DEFAULT_PORT: u16 = 7676;

#[derive(Parser)]
#[command(name = "geisterhand", version, about = "Linux screen automation tool with HTTP API")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the HTTP API server
    Server {
        /// Port to listen on
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,

        /// Target a specific PID
        #[arg(long)]
        pid: Option<i32>,

        /// Target an app by name
        #[arg(long)]
        app: Option<String>,
    },

    /// Launch an app and start a scoped server
    Run {
        /// App name, .desktop file, or path to executable
        app: String,

        /// Port to listen on (auto-assigned if not specified)
        #[arg(short, long)]
        port: Option<u16>,
    },

    /// Take a screenshot and output to stdout or file
    Screenshot {
        /// Output file path (stdout if omitted)
        #[arg(short, long)]
        output: Option<String>,

        /// Image format: png, jpeg
        #[arg(short, long, default_value = "png")]
        format: String,

        /// Target PID
        #[arg(long)]
        pid: Option<i32>,
    },

    /// Click at coordinates or element
    Click {
        /// X coordinate
        #[arg(short)]
        x: Option<f64>,

        /// Y coordinate
        #[arg(short)]
        y: Option<f64>,

        /// Mouse button: left, right, center
        #[arg(short, long, default_value = "left")]
        button: String,

        /// Number of clicks
        #[arg(long, default_value_t = 1)]
        count: u32,

        /// Target element by title
        #[arg(long)]
        title: Option<String>,

        /// Target element by role
        #[arg(long)]
        role: Option<String>,

        /// Target PID
        #[arg(long)]
        pid: Option<i32>,
    },

    /// Type text into the focused element
    Type {
        /// Text to type
        text: String,

        /// Delay between keystrokes in ms
        #[arg(short, long)]
        delay: Option<u64>,

        /// Target PID
        #[arg(long)]
        pid: Option<i32>,
    },

    /// Press a key combination
    Key {
        /// Key name (e.g., "return", "a", "f1")
        key: String,

        /// Modifiers (e.g., "ctrl", "alt", "shift", "super")
        #[arg(short, long, value_delimiter = ',')]
        modifiers: Option<Vec<String>>,

        /// Target PID
        #[arg(long)]
        pid: Option<i32>,
    },

    /// Scroll at position or element
    Scroll {
        /// Horizontal scroll amount
        #[arg(long)]
        dx: Option<f64>,

        /// Vertical scroll amount
        #[arg(long)]
        dy: Option<f64>,

        /// X coordinate
        #[arg(short)]
        x: Option<f64>,

        /// Y coordinate
        #[arg(short)]
        y: Option<f64>,

        /// Target PID
        #[arg(long)]
        pid: Option<i32>,
    },

    /// Query server status
    Status {
        /// Server port
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
    },

    /// Check accessibility support for an app
    Check {
        /// App name or PID
        app: String,
    },

    /// Run as MCP (Model Context Protocol) server over stdio
    Mcp,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Server { port, pid, app } => {
            let target_app = if pid.is_some() || app.is_some() {
                Some(geisterhand::models::api::TargetApp {
                    pid,
                    app_name: app,
                    desktop_file: None,
                })
            } else {
                None
            };
            http::start_server(port, target_app).await?;
        }

        Commands::Run { app, port } => {
            let port = match port {
                Some(p) => p,
                None => http::find_available_port(DEFAULT_PORT).await?,
            };

            // Launch the application
            let child = tokio::process::Command::new("sh")
                .args(["-c", &app])
                .spawn();

            match child {
                Ok(mut child) => {
                    let pid = child.id().map(|p| p as i32);
                    eprintln!("Launched '{}' (PID: {:?})", app, pid);

                    let target_app = Some(geisterhand::models::api::TargetApp {
                        pid,
                        app_name: Some(app),
                        desktop_file: None,
                    });

                    // Start server — it will run until quit or the app exits
                    tokio::select! {
                        result = http::start_server(port, target_app) => {
                            result?;
                        }
                        _ = child.wait() => {
                            eprintln!("Application exited");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to launch '{}': {}", app, e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Screenshot {
            output,
            format,
            pid: _,
        } => {
            use base64::Engine;
            let fmt = geisterhand::screen::ImageFormat::parse(&format);
            let shot = geisterhand::screen::capture_screen(fmt).await?;

            if let Some(path) = output {
                std::fs::write(&path, &shot.data)?;
                eprintln!("Screenshot saved to {} ({}x{})", path, shot.width, shot.height);
            } else {
                // Output base64 to stdout
                let b64 = base64::engine::general_purpose::STANDARD.encode(&shot.data);
                println!("{}", b64);
            }
        }

        Commands::Click {
            x,
            y,
            button,
            count,
            title,
            role,
            pid: _,
        } => {
            let backend = geisterhand::input::get_backend().await?;

            if let (Some(x), Some(y)) = (x, y) {
                backend.mouse_move(x, y)?;
                std::thread::sleep(std::time::Duration::from_millis(10));
                let btn = match button.as_str() {
                    "right" => geisterhand::models::api::MouseButton::Right,
                    "center" => geisterhand::models::api::MouseButton::Center,
                    _ => geisterhand::models::api::MouseButton::Left,
                };
                backend.mouse_click(btn, count)?;
                eprintln!("Clicked at ({}, {}) x{}", x, y, count);
            } else if title.is_some() || role.is_some() {
                // Element click via accessibility
                let query = geisterhand::models::accessibility::ElementQuery {
                    role,
                    title: title.clone(),
                    max_results: Some(1),
                    ..Default::default()
                };
                let result = geisterhand::accessibility::service::find_elements(None, query).await;
                if let Some(elements) = result.elements {
                    if let Some(element) = elements.first() {
                        let resp = geisterhand::accessibility::service::perform_action(
                            element.path.clone(),
                            geisterhand::models::accessibility::AccessibilityAction::Press,
                            None,
                        )
                        .await;
                        if resp.success {
                            eprintln!("Clicked element: {} {:?}", element.role, element.title);
                        } else {
                            eprintln!("Failed to click element: {:?}", resp.error);
                            std::process::exit(1);
                        }
                    }
                } else {
                    eprintln!("No matching element found");
                    std::process::exit(1);
                }
            } else {
                eprintln!("Provide either -x/-y coordinates or --title/--role for element click");
                std::process::exit(1);
            }
        }

        Commands::Type { text, delay, pid: _ } => {
            let backend = geisterhand::input::get_backend().await?;
            backend.type_text(&text, delay)?;
            eprintln!("Typed {} characters", text.len());
        }

        Commands::Key {
            key,
            modifiers,
            pid: _,
        } => {
            let backend = geisterhand::input::get_backend().await?;
            let keycode = geisterhand::input::keycode_map::key_name_to_code(&key)
                .ok_or_else(|| anyhow::anyhow!("Unknown key: '{}'", key))?;

            // Press modifiers
            let modifier_codes: Vec<u16> = modifiers
                .as_ref()
                .map(|mods| {
                    mods.iter()
                        .map(|m| {
                            let km = match m.to_lowercase().as_str() {
                                "ctrl" | "control" => geisterhand::models::api::KeyModifier::Ctrl,
                                "alt" | "option" => geisterhand::models::api::KeyModifier::Alt,
                                "shift" => geisterhand::models::api::KeyModifier::Shift,
                                "super" | "cmd" | "command" => geisterhand::models::api::KeyModifier::Super,
                                _ => geisterhand::models::api::KeyModifier::Ctrl, // fallback
                            };
                            geisterhand::input::keycode_map::modifier_to_code(&km).code()
                        })
                        .collect()
                })
                .unwrap_or_default();

            for &code in &modifier_codes {
                backend.key_down(code)?;
            }
            backend.key_press(keycode.code())?;
            for &code in modifier_codes.iter().rev() {
                backend.key_up(code)?;
            }
            eprintln!("Key press: {} (modifiers: {:?})", key, modifiers);
        }

        Commands::Scroll { dx, dy, x, y, pid: _ } => {
            let backend = geisterhand::input::get_backend().await?;
            if let (Some(x), Some(y)) = (x, y) {
                backend.mouse_move(x, y)?;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            backend.scroll(dx.unwrap_or(0.0), dy.unwrap_or(0.0))?;
            eprintln!("Scrolled dx={}, dy={}", dx.unwrap_or(0.0), dy.unwrap_or(0.0));
        }

        Commands::Status { port } => {
            let url = format!("http://127.0.0.1:{}/status", port);
            eprintln!("Querying {}", url);
            let client = reqwest::Client::new();
            match client.get(&url).send().await {
                Ok(resp) => {
                    let body = resp.text().await?;
                    println!("{}", body);
                }
                Err(e) => {
                    eprintln!("Could not connect to server: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Check { app } => {
            eprintln!("Checking accessibility for '{}'...", app);

            // Check AT-SPI2
            let at_spi_ok = geisterhand::platform::permissions::check_at_spi2_available();
            eprintln!("  AT-SPI2: {}", if at_spi_ok { "available" } else { "NOT available" });

            // Check input
            let uinput_ok = geisterhand::platform::permissions::check_uinput_access();
            eprintln!("  uinput:  {}", if uinput_ok { "accessible" } else { "NOT accessible (XTest will be used)" });

            // Check display
            let display = geisterhand::platform::display::detect_display_server();
            eprintln!("  Display: {:?}", display);

            // Check screenshot
            let screenshot_ok = geisterhand::platform::permissions::check_screen_capture_available();
            eprintln!("  Screen capture: {}", if screenshot_ok { "available" } else { "NOT available" });

            // Try to find the app
            let pid: Option<i32> = app.parse().ok();
            if let Some(pid) = pid {
                let info = geisterhand::platform::process::get_app_info_by_pid(pid);
                match info {
                    Some(info) => eprintln!("  App: {} (PID {})", info.name, info.process_identifier),
                    None => eprintln!("  App: PID {} not found", pid),
                }
            }

            if at_spi_ok {
                eprintln!("\nAccessibility is available. The system is ready.");
            } else {
                eprintln!("\nAT-SPI2 is not running. Enable accessibility in GNOME settings.");
                std::process::exit(1);
            }
        }

        Commands::Mcp => {
            geisterhand::mcp::run_mcp_server().await?;
        }
    }

    Ok(())
}
