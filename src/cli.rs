use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "agy-brain",
    version,
    about = "Universal, standalone cross-platform CLI navigator and session inspector for Antigravity Brains",
    long_about = None
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        help = "Override Antigravity application directory"
    )]
    pub app_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Navigate to brain directory")]
    Go {
        #[arg(help = "Brain index or UUID")]
        target: Option<String>,
    },

    #[command(about = "List brain sessions", aliases = ["ls"])]
    List {
        #[arg(short, long, help = "List all sessions globally")]
        all: bool,
    },

    #[command(about = "Display session details", aliases = ["show"])]
    Info {
        #[arg(help = "Brain index or UUID")]
        target: Option<String>,
    },

    #[command(about = "List artifacts in brain", aliases = ["art"])]
    Artifacts {
        #[arg(help = "Brain index or UUID")]
        target: Option<String>,
    },

    #[command(about = "Open brain folder in OS file manager")]
    Open {
        #[arg(help = "Brain index or UUID")]
        target: Option<String>,
    },

    #[command(about = "Open brain folder in code editor")]
    Edit {
        #[arg(help = "Brain index or UUID")]
        target: Option<String>,
    },

    #[command(about = "Pin brain session to current workspace")]
    Pin {
        #[arg(help = "Brain index or UUID")]
        target: Option<String>,
    },

    #[command(about = "Remove pin for current workspace")]
    Unpin,

    #[command(
        name = "delete",
        aliases = ["rm", "remove"],
        about = "Delete a session, its brain folder and database entry"
    )]
    Delete {
        #[arg(help = "Brain index or UUID to delete")]
        target: Option<String>,

        #[arg(short, long, help = "Skip interactive confirmation prompt")]
        force: bool,
    },

    #[command(about = "Print absolute brain folder path")]
    Resolve {
        #[arg(help = "Brain index or UUID")]
        target: Option<String>,
    },

    #[command(about = "Launch standalone interactive TUI")]
    Tui,

    #[command(about = "Generate shell integration code")]
    Init {
        #[arg(help = "Shell name (bash, zsh, fish, powershell)")]
        shell: String,
    },

    #[command(about = "Install shell integration into RC/profile")]
    Install {
        #[arg(help = "Target shell (bash, zsh, fish, powershell)")]
        shell: Option<String>,
    },

    #[command(about = "Check system health, database and tools")]
    Doctor,
}
