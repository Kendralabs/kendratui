//! CLI argument definitions (clap).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// KendraCLI — AI-powered command-line tool for accelerated development.
#[derive(Parser, Debug)]
#[command(
    name = "kendra",
    version,
    about = "KendraCLI — AI-powered command-line tool for accelerated development",
    long_about = None,
    after_help = "Examples:\n  \
        kendra                          Start interactive CLI session\n  \
        kendra \"do something\"           Start session with initial message\n  \
        kendra -p \"create hello.py\"     Non-interactive mode\n  \
        kendra --continue               Resume most recent session\n  \
        kendra run ui                   Start web UI\n  \
        kendra mcp list                 List MCP servers"
)]
pub struct Cli {
    /// Execute a single prompt and exit (non-interactive mode).
    #[arg(short, long, value_name = "TEXT")]
    pub prompt: Option<String>,

    /// Set working directory (defaults to current directory).
    #[arg(short = 'd', long = "working-dir", value_name = "PATH")]
    pub working_dir: Option<PathBuf>,

    /// Enable verbose output with detailed logging.
    #[arg(short, long)]
    pub verbose: bool,

    /// Resume the most recent session for the current working directory.
    #[arg(short = 'c', long = "continue")]
    pub continue_session: bool,

    /// Resume a session (optionally specify ID, or pick interactively).
    #[arg(short = 'r', long, value_name = "SESSION_ID")]
    pub resume: Option<Option<String>>,

    /// Skip all permission prompts and auto-approve every operation.
    #[arg(long)]
    pub dangerously_skip_permissions: bool,

    /// Color theme for the TUI (dark, light, dracula). Auto-detected if not set.
    #[arg(long, value_name = "THEME")]
    pub theme: Option<String>,

    /// Configuration profile to use (dev, prod, fast).
    #[arg(long, value_name = "PROFILE")]
    pub profile: Option<String>,

    /// Set the session title (for non-interactive mode).
    #[arg(long, value_name = "TITLE")]
    pub title: Option<String>,

    /// Select which agent handles the session (e.g. "general", "explore").
    #[arg(long, value_name = "AGENT")]
    pub agent: Option<String>,

    /// Replay a recorded event JSONL file for debugging.
    /// Record events by setting KENDRA_DEBUG_EVENTS=1.
    #[arg(long, value_name = "JSONL_FILE")]
    pub replay: Option<PathBuf>,

    /// Initial message to start the session with (positional).
    #[arg(value_name = "MESSAGE")]
    pub message: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Top-level subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the interactive setup wizard (first-run or re-configure).
    Setup,

    /// Manage KendraCLI configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Configure and manage MCP servers.
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },

    /// Run development tools.
    Run {
        #[command(subcommand)]
        action: RunAction,
    },

    /// Manage conversation sessions.
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Manage channel integrations (Telegram, etc.).
    Channel {
        #[command(subcommand)]
        action: ChannelAction,
    },

    /// Start a headless remote session (Telegram as primary interface).
    Remote {
        /// Resume the most recent session instead of starting a new one.
        #[arg(short = 'c', long = "continue")]
        continue_session: bool,

        /// Resume a specific session by ID.
        #[arg(short = 'r', long, value_name = "SESSION_ID")]
        resume: Option<String>,
    },

    /// Manage marketplaces for plugins and skills.
    Marketplace {
        #[command(subcommand)]
        action: MarketplaceAction,
    },

    /// Manage installed plugins.
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

/// Channel subcommands.
#[derive(Subcommand, Debug)]
pub enum ChannelAction {
    /// Add Telegram bot (prompts for token if not provided).
    Add {
        /// Bot token (optional — will prompt interactively if not provided).
        token: Option<String>,
    },
    /// Remove Telegram bot configuration.
    Remove,
    /// Show channel status and paired users.
    Status,
    /// Run Telegram bot (background by default, --foreground to keep in terminal).
    Serve {
        /// Run in foreground instead of background.
        #[arg(long, short)]
        foreground: bool,
    },
    /// Approve a Telegram user by ID.
    Pair {
        /// Telegram user ID to approve.
        user_id: String,
    },
    /// Revoke a Telegram user's access.
    Unpair {
        /// Telegram user ID to remove.
        user_id: String,
    },
}

/// Config subcommands.
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Run the interactive setup wizard.
    Setup,
    /// Display current configuration.
    Show,
}

/// MCP subcommands.
#[derive(Subcommand, Debug)]
pub enum McpAction {
    /// List all configured MCP servers.
    List,
    /// Show detailed information about a specific server.
    Get {
        /// Server name.
        name: String,
    },
    /// Add a new MCP server.
    Add {
        /// Unique name for the server.
        name: String,
        /// Command to start the server (e.g., "uvx", "node", "python").
        command: String,
        /// Arguments to pass to the command.
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
        /// Environment variables (KEY=VALUE).
        #[arg(long, value_name = "KEY=VALUE")]
        env: Vec<String>,
        /// Don't auto-start this server on launch.
        #[arg(long)]
        no_auto_start: bool,
    },
    /// Remove an MCP server.
    Remove {
        /// Server name.
        name: String,
    },
    /// Enable an MCP server.
    Enable {
        /// Server name.
        name: String,
    },
    /// Disable an MCP server.
    Disable {
        /// Server name.
        name: String,
    },
}

/// Session subcommands.
#[derive(Subcommand, Debug)]
pub enum SessionAction {
    /// List recent sessions.
    List {
        /// Include archived sessions.
        #[arg(long)]
        archived: bool,
        /// Maximum number of sessions to show.
        #[arg(short = 'n', long, default_value_t = 20)]
        max_count: usize,
        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Delete a session permanently.
    Delete {
        /// Session ID to delete.
        id: String,
    },
    /// Export a session as JSON.
    Export {
        /// Session ID to export. Defaults to the most recent session.
        id: Option<String>,
    },
}

/// Run subcommands.
#[derive(Subcommand, Debug)]
pub enum RunAction {
    /// Start the web UI (backend + frontend).
    Ui {
        /// Port for backend API server.
        #[arg(long, default_value_t = 8080)]
        ui_port: u16,
        /// Host for backend API server.
        #[arg(long, default_value = "127.0.0.1")]
        ui_host: String,
    },
}

/// Marketplace subcommands.
#[derive(Subcommand, Debug)]
pub enum MarketplaceAction {
    /// Add a new marketplace.
    Add {
        /// Git URL of the marketplace.
        url: String,
        /// Optional name for the marketplace.
        name: Option<String>,
        /// Git branch to track.
        #[arg(long, default_value = "main")]
        branch: String,
    },
    /// Remove a marketplace.
    Remove {
        /// Marketplace name.
        name: String,
    },
    /// List all registered marketplaces.
    List,
    /// Sync (git pull) a marketplace.
    Sync {
        /// Optional marketplace name. If not provided, syncs all marketplaces.
        name: Option<String>,
    },
    /// Search for plugins in marketplaces.
    Search {
        /// Search query.
        query: String,
    },
    /// List plugins available in a marketplace.
    Plugins {
        /// Marketplace name.
        name: String,
    },
}

/// Plugin subcommands.
#[derive(Subcommand, Debug)]
pub enum PluginAction {
    /// Install a plugin.
    Install {
        /// Name of the plugin.
        name: String,
        /// Name of the marketplace containing the plugin.
        marketplace: String,
        /// Install globally (~/.kendra/plugins) instead of project-locally.
        #[arg(long, short)]
        global: bool,
    },
    /// Uninstall a plugin.
    Uninstall {
        /// Name of the plugin.
        name: String,
        /// Name of the marketplace the plugin was installed from.
        marketplace: String,
        /// Uninstall from global scope instead of project scope.
        #[arg(long, short)]
        global: bool,
    },
    /// List installed plugins.
    List {
        /// Filter by scope (global/project). If not specified, lists all.
        #[arg(long, short)]
        scope: Option<String>,
    },
    /// Enable a disabled plugin.
    Enable {
        /// Name of the plugin.
        name: String,
        /// Name of the marketplace the plugin was installed from.
        marketplace: String,
        /// Scope of the plugin (global/project).
        #[arg(long, short)]
        global: bool,
    },
    /// Disable a plugin.
    Disable {
        /// Name of the plugin.
        name: String,
        /// Name of the marketplace the plugin was installed from.
        marketplace: String,
        /// Scope of the plugin (global/project).
        #[arg(long, short)]
        global: bool,
    },
}
