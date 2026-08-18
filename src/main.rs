//! termbrowse — structure-first terminal web session.
//!
//! Fetch → parse roles → 256-color layout → ratatui.
//! No browser engine. No page JS.

mod art;
mod color;
mod fetch;
mod history;
mod home;
mod keys;
mod layout;
mod math;
mod md;
mod model;
mod parse;
mod serp;
mod session;
mod snapshot;
mod theme;
mod tui;
mod update;
mod urlutil;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use layout::layout_document;
use session::load_page;
use snapshot::{snapshot, to_json};
use urlutil::ensure_http_url;

#[derive(Parser, Debug)]
#[command(
    name = "browse",
    about = "Terminal web session — structure-first, 256-color identity, no Chrome",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Open this URL (omit to show Start Page)
    url: Option<String>,

    /// Terminal width for snapshot/text layout
    #[arg(long, default_value_t = 100)]
    width: u16,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start Page (favorites + reading list)
    Home,
    /// Open a URL in the interactive session
    Open { url: String },
    /// Agent JSON snapshot of the structured page
    Snapshot {
        url: String,
        #[arg(long, default_value_t = 100)]
        width: u16,
    },
    /// Plain text layout of the structured page
    Text {
        url: String,
        #[arg(long, default_value_t = 100)]
        width: u16,
    },
    /// Replace this binary with the latest GitHub Release
    Update,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Home) => tui::run_home().await?,
        Some(Commands::Open { url }) => {
            tui::run(&ensure_http_url(&url)?).await?;
        }
        Some(Commands::Snapshot { url, width }) => {
            let url = ensure_http_url(&url)?;
            let page = load_page(&url).await?;
            let lay = layout_document(&page.doc, width);
            eprintln!(
                "total_ms={} text_len={}",
                page.total_ms,
                page.doc.text_len()
            );
            println!("{}", to_json(&snapshot(&page.doc, Some(&lay)))?);
        }
        Some(Commands::Text { url, width }) => {
            let url = ensure_http_url(&url)?;
            let page = load_page(&url).await?;
            let lay = layout_document(&page.doc, width);
            let snap = snapshot(&page.doc, Some(&lay));
            if let Some(layout) = snap.layout {
                print!("{}", layout.text);
            }
        }
        Some(Commands::Update) => {
            let dest = std::env::current_exe().context("current exe")?;
            let dest = dest.canonicalize().unwrap_or(dest);
            match update::run_update(env!("CARGO_PKG_VERSION"), &dest).await? {
                update::UpdateOutcome::AlreadyLatest { version } => {
                    println!("browse is up to date ({version})");
                }
                update::UpdateOutcome::Updated { from, to } => {
                    println!("updated {from} → {to}");
                }
            }
        }
        None => match cli.url {
            None => tui::run_home().await?,
            Some(url) => tui::run(&ensure_http_url(&url)?).await?,
        },
    }

    Ok(())
}
