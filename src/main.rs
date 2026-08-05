//! termbrowse — terminal browser with two modes:
//! - **full** (default): Chrome loads the page; terminal paints it **CRT-style**
//!   line-by-line (scanlines). Not HD — honest terminal graphics.
//! - **lite**: HTML-only document browser (no JS, fast, no Chrome).

mod chrome;
mod fetch;
mod layout;
mod model;
mod parse;
mod snapshot;
mod term_image;
mod tui;
mod tui_full;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use fetch::fetch_url;
use layout::layout_document;
use parse::parse_html;
use snapshot::{snapshot, to_json};
use term_image::Phosphor;

#[derive(Parser, Debug)]
#[command(
    name = "termbrowse",
    about = "Terminal browser — CRT scanline previews (Chrome) or lite HTML mode",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Open this URL in the interactive TUI
    url: Option<String>,

    /// HTML-only mode (no Chrome, no JS — broken for YouTube-class sites)
    #[arg(long, global = true)]
    lite: bool,

    /// CRT phosphor: color | green | amber | mono (full mode)
    #[arg(long, global = true, default_value = "green")]
    phosphor: String,

    /// Terminal width for layout when not in a TTY (lite snapshot)
    #[arg(long, default_value_t = 100)]
    width: u16,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Open a URL in the interactive terminal UI
    Open { url: String },
    /// Lite: fetch + parse + print agent JSON snapshot
    Snapshot {
        url: String,
        #[arg(long, default_value_t = true)]
        text: bool,
        #[arg(long, default_value_t = 100)]
        width: u16,
    },
    /// Lite: fetch + parse + print plain text
    Text {
        url: String,
        #[arg(long, default_value_t = 100)]
        width: u16,
    },
    /// Full mode: load page in Chrome, save screenshot PNG (no TUI)
    Dump {
        url: String,
        /// Output PNG path
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

    let phosphor = Phosphor::parse(&cli.phosphor).unwrap_or(Phosphor::Green);

    match cli.command {
        Some(Commands::Open { url }) => open_tui(&url, cli.lite, phosphor).await?,
        Some(Commands::Snapshot { url, text, width }) => {
            let doc = load_lite(&url).await?;
            let lay = layout_document(&doc, width);
            let mut snap = snapshot(&doc, if text { Some(&lay) } else { None });
            if !text {
                snap.layout = None;
            }
            println!("{}", to_json(&snap)?);
        }
        Some(Commands::Text { url, width }) => {
            let doc = load_lite(&url).await?;
            let lay = layout_document(&doc, width);
            let snap = snapshot(&doc, Some(&lay));
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
                    "ok: title={:?} links={} bytes={} load_ms={} → {}",
                    frame.doc.title,
                    frame.doc.links.len(),
                    frame.png.len(),
                    frame.load_ms,
                    out_path
                );
                for (i, l) in frame.doc.links.iter().take(8).enumerate() {
                    eprintln!("  [{}] {} → {}", i + 1, l.text, l.href);
                }
                Ok(())
            })
            .await
            .context("dump task")??;
        }
        None => {
            let url = cli
                .url
                .context("usage: termbrowse <url>   (add --lite for HTML-only)")?;
            open_tui(&url, cli.lite, phosphor).await?;
        }
    }

    Ok(())
}

async fn load_lite(url: &str) -> Result<model::Document> {
    let fetched = fetch_url(url).await?;
    Ok(parse_html(&fetched.url, &fetched.body, fetched.fetch_ms))
}

async fn open_tui(url: &str, lite: bool, phosphor: Phosphor) -> Result<()> {
    let url = chrome::ensure_http_url(url)?;
    if lite {
        let doc = load_lite(&url).await?;
        tui::run(doc).await
    } else {
        // Chrome + CRT paint are sync; run off the async runtime.
        tokio::task::spawn_blocking(move || tui_full::run(&url, phosphor))
            .await
            .context("full browser task failed")?
    }
}
