mod app;
mod chart;
mod data;

use clap::builder::PossibleValuesParser;
use clap::Parser;
use gpui::*;
use gpui_component::Root;
use gpui_component_assets::Assets;

use crate::app::ChartWindow;

#[derive(Parser)]
#[command(name = "scgp", about = "Standalone Candlestick Chart Viewer")]
struct Cli {
    /// Ticker symbol (e.g. AAPL, MSFT, TSLA)
    #[arg(short, long)]
    ticker: Option<String>,

    /// Maximize window
    #[arg(short = 'm', long, default_value_t = false)]
    maximized: bool,

    /// Window size WxH (e.g. 1920x1080)
    #[arg(short, long, default_value = "1920x1080")]
    size: String,

    /// Candle interval (e.g. 1d, 1h, 5m)
    #[arg(short, long, default_value = "1d",
        value_parser = PossibleValuesParser::new([
            "1m", "2m", "5m", "15m", "30m", "60m", "90m", "1h", "1d", "5d", "1wk", "1mo", "3mo"
        ]))]
    interval: String,

    /// Data range (e.g. 1y, 6mo, 1mo, 15y, max)
    #[arg(short, long, default_value = "1y")]
    range: String,
}

fn parse_size(s: &str) -> (f32, f32) {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() == 2 {
        let w = parts[0].parse::<f32>().unwrap_or(1920.0).clamp(200.0, 7680.0);
        let h = parts[1].parse::<f32>().unwrap_or(1080.0).clamp(200.0, 4320.0);
        (w, h)
    } else {
        (1920.0, 1080.0)
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error,scgp=info")),
        )
        .init();

    let cli = Cli::parse();

    let ticker = cli.ticker.map(|t| t.to_uppercase());
    let interval = cli.interval.clone();
    let range = cli.range.clone();
    let maximized = cli.maximized;
    let (win_w, win_h) = parse_size(&cli.size);

    tracing::info!("scgp: {:?} interval={} range={}", ticker, interval, range);

    // Create Tokio runtime (leaked to keep alive for the lifetime of the process)
    let rt = Box::leak(Box::new(
        tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime"),
    ));
    let tokio_handle = rt.handle().clone();

    let app = Application::new().with_assets(Assets);

    app.run(move |cx| {
        gpui_component::init(cx);

        // Compute window bounds in synchronous context where primary_display is available
        let bounds = Bounds {
            origin: point(px(100.0), px(100.0)),
            size: size(px(win_w), px(win_h)),
        };
        let window_bounds = if maximized {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        };

        let tokio_handle = tokio_handle.clone();
        let ticker = ticker.clone();
        let interval = interval.clone();
        let range = range.clone();

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(window_bounds),
                    ..WindowOptions::default()
                },
                |window, cx| {
                    gpui_component::theme::Theme::change(
                        gpui_component::theme::ThemeMode::Dark,
                        Some(window),
                        cx,
                    );
                    {
                        let theme = gpui_component::theme::Theme::global_mut(cx);
                        theme.colors.background = gpui::black();
                        theme.colors.foreground = gpui::white();
                    }
                    let title = match &ticker {
                        Some(t) => format!("scgp - {}", t),
                        None => "scgp".to_string(),
                    };
                    window.set_window_title(&title);
                    let chart_window = cx.new(|cx| {
                        ChartWindow::new(
                            ticker.clone(),
                            interval.clone(),
                            range.clone(),
                            tokio_handle.clone(),
                            window,
                            cx,
                        )
                    });
                    cx.new(|cx| Root::new(chart_window, window, cx))
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
