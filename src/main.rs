//! termbrowse — custom interactive terminal browser.
//!
//! Pure stack: HTTPS fetch → HTML parse → cell layout → Grok-density TUI.
//! No headless Chrome. No screenshot paint. Same Document for humans + agents.

mod fetch;
mod layout;
mod model;
mod parse;
mod session;
mod snapshot;
mod theme;
mod tui_session;
mod urlutil;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use layout::layout_document;
use session::{load_page, LoadSource};
use snapshot::{snapshot, to_json};
use urlutil::ensure_http_url;

#[derive(Parser, Debug)]
#[command(
    name = "termbrowse",
    about = "Custom terminal web session — structure browser, no Chrome",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Open this URL in the interactive TUI
    url: Option<String>,

    /// Terminal width for snapshot/text layout
    #[arg(long, default_value_t = 100)]
    width: u16,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Open a URL in the interactive session TUI
    Open { url: String },
    /// Agent JSON snapshot of the structured page
    Snapshot {
        url: String,
        #[arg(long, default_value_t = true)]
        text: bool,
        #[arg(long, default_value_t = 100)]
        width: u16,
    },
    /// Plain text layout of the structured page
    Text {
        url: String,
        #[arg(long, default_value_t = 100)]
        width: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Open { url }) => {
            let url = ensure_http_url(&url)?;
            tui_session::run(&url).await?;
        }
        Some(Commands::Snapshot { url, text, width }) => {
            let url = ensure_http_url(&url)?;
            let page = load_page(&url).await?;
            let lay = layout_document(&page.doc, width);
            let mut snap = snapshot(&page.doc, if text { Some(&lay) } else { None });
            if !text {
                snap.layout = None;
            }
            let src = match page.source {
                LoadSource::Structure => "structure",
            };
            eprintln!(
                "source={src} total_ms={} text_len={}",
                page.total_ms,
                page.doc.text_len()
            );
            println!("{}", to_json(&snap)?);
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
        None => {
            let url = cli
                .url
                .context("usage: termbrowse <url>\n  termbrowse snapshot <url>\n  termbrowse text <url>")?;
            let url = ensure_http_url(&url)?;
            tui_session::run(&url).await?;
        }
    }

    Ok(())
}
