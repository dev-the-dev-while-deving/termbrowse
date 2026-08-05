//! termbrowse — interactive session browser for the terminal.
//!
//! Structure-first (fast HTML → blocks). Escalates to Chrome **extract** only when
//! the page is a thin JS shell. Same Document model for humans and agents.
//! Pixel modes (`--pixels`) are optional; not the product default.

mod chrome;
mod fetch;
mod layout;
mod model;
mod parse;
mod session;
mod snapshot;
mod term_image;
mod theme;
mod tui;
mod tui_full;
mod tui_session;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use fetch::fetch_url;
use layout::layout_document;
use parse::parse_html;
use session::{load_page, LoadSource};
use snapshot::{snapshot, to_json};
use term_image::Phosphor;

#[derive(Parser, Debug)]
#[command(
    name = "termbrowse",
    about = "Interactive terminal web session — structure-first, escalate when needed",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Open this URL in the interactive TUI
    url: Option<String>,

    /// Never escalate to Chrome (structure / HTML only)
    #[arg(long, global = true)]
    structure_only: bool,

    /// Legacy: pixel paint via Kitty/CRT (not the default product path)
    #[arg(long, global = true)]
    pixels: bool,

    /// CRT phosphor when using --pixels
    #[arg(long, global = true, default_value = "green")]
    phosphor: String,

    /// Terminal width for snapshot/text layout
    #[arg(long, default_value_t = 100)]
    width: u16,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Open a URL in the interactive session TUI
    Open { url: String },
    /// Agent JSON snapshot (structure-first + escalate)
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
    /// Legacy: Chrome screenshot PNG
    Dump {
        url: String,
        #[arg(short, long, default_value = "page.png")]
        out: String,
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
    let escalate = !cli.structure_only;
    let phosphor = Phosphor::parse(&cli.phosphor).unwrap_or(Phosphor::Green);

    match cli.command {
        Some(Commands::Open { url }) => open_tui(&url, escalate, cli.pixels, phosphor).await?,
        Some(Commands::Snapshot { url, text, width }) => {
            let url = chrome::ensure_http_url(&url)?;
            let mut browser = None;
            let page = load_page(&url, escalate, &mut browser).await?;
            let lay = layout_document(&page.doc, width);
            let mut snap = snapshot(&page.doc, if text { Some(&lay) } else { None });
            if !text {
                snap.layout = None;
            }
            // Annotate source for agents.
            let src = match page.source {
                LoadSource::Structure => "structure",
                LoadSource::Escalated => "escalated",
            };
            eprintln!(
                "source={src} total_ms={} text_len={}",
                page.total_ms,
                page.doc.text_len()
            );
            println!("{}", to_json(&snap)?);
        }
        Some(Commands::Text { url, width }) => {
            let url = chrome::ensure_http_url(&url)?;
            let mut browser = None;
            let page = load_page(&url, escalate, &mut browser).await?;
            let lay = layout_document(&page.doc, width);
            let snap = snapshot(&page.doc, Some(&lay));
            if let Some(layout) = snap.layout {
                print!("{}", layout.text);
            }
        }
        Some(Commands::Dump { url, out }) => {
            let url = chrome::ensure_http_url(&url)?;
            let out_path = out.clone();
            tokio::task::spawn_blocking(move || -> Result<()> {
                let browser = chrome::FullBrowser::launch()?;
                eprintln!("loading {url} …");
                let frame = browser.open(&url)?;
                std::fs::write(&out_path, &frame.png)
                    .with_context(|| format!("write {out_path}"))?;
                eprintln!(
                    "ok: title={:?} links={} bytes={} → {}",
                    frame.doc.title,
                    frame.doc.links.len(),
                    frame.png.len(),
                    out_path
                );
                Ok(())
            })
            .await
            .context("dump task")??;
        }
        None => {
            let url = cli.url.context(
                "usage: termbrowse <url>\n  --structure-only   never use Chrome\n  --pixels           legacy Kitty/CRT paint",
            )?;
            open_tui(&url, escalate, cli.pixels, phosphor).await?;
        }
    }

    Ok(())
}

async fn open_tui(url: &str, escalate: bool, pixels: bool, phosphor: Phosphor) -> Result<()> {
    let url = chrome::ensure_http_url(url)?;
    if pixels {
        tokio::task::spawn_blocking(move || tui_full::run(&url, phosphor))
            .await
            .context("pixels mode")?
    } else {
        tui_session::run(&url, escalate).await
    }
}

#[allow(dead_code)]
async fn load_lite(url: &str) -> Result<model::Document> {
    let fetched = fetch_url(url).await?;
    Ok(parse_html(&fetched.url, &fetched.body, fetched.fetch_ms))
}
