# scgp

Standalone GPU-accelerated candlestick chart viewer. Pulls market data from Yahoo Finance and renders interactive charts using [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).

![screenshot](assets/Screenshot_20260208_232808.png)

## Features

- Canvas-based candlestick rendering with volume bars
- Smooth zoom (scroll wheel) with cursor-anchored scaling
- Drag panning across historical data
- Crosshair with OHLCV tooltip
- Price and time axes with cursor tracking labels
- Interactive settings overlay: switch ticker, interval, and range on the fly
- Full history support (`max` range)
- Configurable via CLI or in-app settings

## Installation

```sh
cargo install --git https://github.com/7jrxt42BxFZo4iAnN4CX/scgp
```

Or build from source:

```sh
git clone https://github.com/7jrxt42BxFZo4iAnN4CX/scgp
cd scgp
cargo build --release
```

The binary will be at `target/release/scgp`.

## Usage

```
scgp                                   # opens with ticker input dialog
scgp -t AAPL                           # loads AAPL daily 1y
scgp -t MSFT -i 1h -r 730d            # hourly data for ~2 years
scgp -t TSLA -i 5m -r 60d -s 2560x1440
scgp -t BTC-USD -i 1d -r max -m       # full history, maximized
```

### CLI arguments

| Flag | Long | Default | Description |
|------|------|---------|-------------|
| `-t` | `--ticker` | *optional* | Ticker symbol (e.g. AAPL, MSFT, BTC-USD). Opens input dialog if omitted |
| `-i` | `--interval` | `1d` | Candle interval (1m, 5m, 15m, 30m, 1h, 1d, 1wk, 1mo) |
| `-r` | `--range` | `1y` | Data range (1d, 5d, 1mo, 3mo, 6mo, 1y, 2y, 5y, max, or Nd e.g. 60d, 730d) |
| `-s` | `--size` | `1920x1080` | Window size WxH |
| `-m` | `--maximized` | `false` | Maximize window |

### Yahoo Finance range limits

| Interval | Max range |
|----------|-----------|
| 1m | 7 days |
| 5m, 15m, 30m | 60 days |
| 1h | 730 days |
| 1d, 1wk, 1mo | unlimited (max) |

## Keybindings

| Key | Action |
|-----|--------|
| Escape | Toggle settings overlay (ticker, interval, range) |
| Enter | Load ticker (empty input reloads current with new settings) |
| ? | Toggle help |
| Scroll wheel | Zoom in/out (anchored to cursor) |
| Left mouse drag | Pan through data |

## Building from source

### Prerequisites

- Rust 2024 edition (1.85+)
- System dependencies required by GPUI:
  - **Linux**: `libxcb`, `libxkbcommon`, `libwayland`, `vulkan-loader` and related dev packages
  - **macOS**: Xcode command line tools

```sh
cargo build --release
```

## License

[MIT](LICENSE)
