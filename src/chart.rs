use std::sync::{Arc, RwLock};

use gpui::{prelude::*, *};

use crate::data::Quote;

/// Returns (body_width, wick_width) based on the number of visible candles.
fn candle_dimensions(candles_per_screen: usize) -> (f32, f32) {
    // (max_candles, body_width, wick_width)
    const THRESHOLDS: &[(usize, f32, f32)] = &[
        (50, 20.0, 3.0),
        (100, 15.0, 3.0),
        (200, 8.0, 2.0),
        (350, 5.0, 2.0),
    ];
    for &(max, body, wick) in THRESHOLDS {
        if candles_per_screen <= max {
            return (body, wick);
        }
    }
    (3.0, 1.0)
}

struct CandleGeometry {
    wick_bounds: Bounds<Pixels>,
    body_bounds: Bounds<Pixels>,
    is_bullish: bool,
}

struct VolumeBarGeometry {
    bounds: Bounds<Pixels>,
    is_bullish: bool,
}

struct ChartPaintData {
    candles: Vec<CandleGeometry>,
    volume_bars: Vec<VolumeBarGeometry>,
}

fn calculate_dimensions(visible_quotes: &[Quote]) -> (f64, f64) {
    if visible_quotes.is_empty() {
        return (0.0, 100.0);
    }

    let mut min_price = f64::MAX;
    let mut max_price = f64::MIN;

    for quote in visible_quotes {
        min_price = min_price.min(quote.low.into_inner());
        max_price = max_price.max(quote.high.into_inner());
    }

    let padding = (max_price - min_price) * 0.05;
    min_price -= padding;
    max_price += padding;

    (min_price, max_price)
}

/// Pick a date format based on the time span of visible data.
fn time_axis_format(visible_quotes: &[Quote]) -> &'static str {
    if visible_quotes.len() < 2 {
        return "%Y-%m-%d %H:%M";
    }
    let first = visible_quotes.first().unwrap().time;
    let last = visible_quotes.last().unwrap().time;
    let span_days = (last - first).num_days();
    if span_days > 365 {
        "%Y-%m"
    } else if span_days > 5 {
        "%Y-%m-%d"
    } else {
        "%m/%d %H:%M"
    }
}

fn render_time_axis(
    visible_quotes: &[Quote],
    cursor_time: Option<chrono::DateTime<chrono::Utc>>,
    cursor_x_pct: Option<f32>,
) -> Div {
    if visible_quotes.is_empty() {
        return div();
    }

    let fmt = time_axis_format(visible_quotes);

    let max_labels = 6;
    let len = visible_quotes.len();
    let step = (len / max_labels).max(1);

    let indices: Vec<usize> = (0..len).step_by(step).take(max_labels).collect();

    let mut axis = div()
        .h(px(30.))
        .w_full()
        .flex()
        .flex_row()
        .justify_between()
        .px_2()
        .bg(rgb(0x000000))
        .border_t_1()
        .border_color(rgb(0x3e3e3e))
        .relative()
        .children(indices.into_iter().filter_map(|i| {
            visible_quotes.get(i).map(|quote| {
                div()
                    .text_sm()
                    .text_color(rgb(0xa0a0a0))
                    .child(quote.time.format(fmt).to_string())
            })
        }));

    if let (Some(time), Some(x_pct)) = (cursor_time, cursor_x_pct) {
        axis = axis.child(
            div()
                .absolute()
                .left(relative(x_pct / 100.0))
                .top(px(0.))
                .bottom(px(0.))
                .ml(px(-45.))
                .w(px(90.))
                .bg(rgb(0x1e40af))
                .border_1()
                .border_color(rgba(0x60a5fa80))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0xffffff))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(time.format(fmt).to_string()),
                ),
        );
    }

    axis
}

fn render_price_axis(min_price: f64, max_price: f64, cursor_price: Option<f64>) -> Div {
    let price_range = max_price - min_price;

    let mut axis = div()
        .w(px(80.))
        .h_full()
        .flex()
        .flex_col()
        .justify_between()
        .py_2()
        .px_2()
        .bg(rgb(0x000000))
        .border_l_1()
        .border_color(rgb(0x3e3e3e))
        .relative();

    if price_range == 0.0 {
        // Flat-line: show single centered price label
        let center_price = (min_price + max_price) / 2.0;
        axis = axis
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xa0a0a0))
                    .text_right()
                    .child(format!("${:.2}", center_price)),
            );
    } else {
        let label_count = 8;
        let step = price_range / label_count as f64;
        axis = axis.children((0..=label_count).map(|i| {
            let price = max_price - (step * i as f64);
            div()
                .text_sm()
                .text_color(rgb(0xa0a0a0))
                .text_right()
                .child(format!("${:.2}", price))
        }));
    }

    if let Some(price) = cursor_price {
        let y_pct = ((max_price - price) / price_range * 100.0) as f32;
        axis = axis.child(
            div()
                .absolute()
                .left(px(0.))
                .right(px(0.))
                .top(relative(y_pct / 100.0))
                .h(px(22.))
                .mt(px(-11.))
                .bg(rgb(0x1e40af))
                .border_1()
                .border_color(rgba(0x60a5fa80))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0xffffff))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(format!("${:.2}", price)),
                ),
        );
    }

    axis
}

fn render_crosshair_overlay(
    quote: &Quote,
    mouse_pos: Point<Pixels>,
    chart_bounds: Option<Bounds<Pixels>>,
) -> Div {
    let is_bullish = quote.close >= quote.open;
    let color = if is_bullish {
        rgb(0x4ade80)
    } else {
        rgb(0xf87171)
    };

    let (adjusted_x, adjusted_y) = if let Some(bounds) = chart_bounds {
        let adj_x = mouse_pos.x - bounds.origin.x;
        let adj_y = mouse_pos.y - bounds.origin.y;
        (adj_x, adj_y)
    } else {
        (mouse_pos.x, mouse_pos.y)
    };

    div()
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .right(px(0.))
        .bottom(px(0.))
        .child(
            div()
                .absolute()
                .top(px(0.))
                .bottom(px(0.))
                .left(adjusted_x)
                .w(px(1.))
                .bg(rgba(0xcccccc40)),
        )
        .child(
            div()
                .absolute()
                .left(px(0.))
                .right(px(0.))
                .top(adjusted_y)
                .h(px(1.))
                .bg(rgba(0xcccccc40)),
        )
        .child(
            div()
                .absolute()
                .top(px(10.))
                .left(px(10.))
                .p_2()
                .bg(rgba(0x0a0a0acc))
                .border_1()
                .border_color(rgb(0x3e3e3e))
                .rounded_md()
                .flex()
                .flex_col()
                .gap_1()
                .text_sm()
                .child(
                    div()
                        .text_color(rgb(0xe0e0e0))
                        .child(quote.time.format("%Y-%m-%d %H:%M").to_string()),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .text_color(rgb(0xe0e0e0))
                        .child(format!("O: ${:.2}", quote.open.into_inner()))
                        .child(format!("H: ${:.2}", quote.high.into_inner())),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .text_color(rgb(0xe0e0e0))
                        .child(format!("L: ${:.2}", quote.low.into_inner()))
                        .child(
                            div()
                                .text_color(color)
                                .child(format!("C: ${:.2}", quote.close.into_inner())),
                        ),
                )
                .child(
                    div()
                        .text_color(rgb(0x808080))
                        .child(format!("Vol: {:.0}", quote.volume.into_inner())),
                ),
        )
}

#[derive(IntoElement)]
pub struct CandlestickChart {
    quotes: Arc<Vec<Quote>>,
    candles_per_screen: usize,
    scroll_offset: usize,
    crosshair_position: Option<Point<Pixels>>,
    hovered_quote_index: Option<usize>,
    chart_bounds_shared: Arc<RwLock<Option<Bounds<Pixels>>>>,
    ticker: String,
    interval: String,
    range: String,
}

impl CandlestickChart {
    pub fn new(
        quotes: Arc<Vec<Quote>>,
        scroll_offset: usize,
        candles_per_screen: usize,
        crosshair_position: Option<Point<Pixels>>,
        hovered_quote_index: Option<usize>,
        chart_bounds_shared: Arc<RwLock<Option<Bounds<Pixels>>>>,
        ticker: String,
        interval: String,
        range: String,
    ) -> Self {
        Self {
            quotes,
            candles_per_screen,
            scroll_offset,
            crosshair_position,
            hovered_quote_index,
            chart_bounds_shared,
            ticker,
            interval,
            range,
        }
    }
}

impl RenderOnce for CandlestickChart {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let Self {
            quotes,
            candles_per_screen,
            scroll_offset,
            crosshair_position,
            hovered_quote_index,
            chart_bounds_shared,
            ticker,
            interval,
            range,
        } = self;

        if quotes.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child("No data available")
                .into_any_element();
        }

        let start_idx = scroll_offset.min(quotes.len());
        let end_idx = (scroll_offset + candles_per_screen).min(quotes.len());
        let display_quotes = &quotes[start_idx..end_idx];

        let (min_price, max_price) = calculate_dimensions(display_quotes);

        // Header line: "AAPL · 1d · 1y"
        let header = div()
            .h(px(28.))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .gap_1()
            .bg(rgb(0x000000))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xffffff))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ticker),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x707070))
                    .child(format!(" · {} · {}", interval, range)),
            );

        // Clone/copy values needed by canvas closures
        let chart_bounds_for_canvas = chart_bounds_shared.clone();
        let quotes_for_canvas = quotes.clone();
        let canvas_start = start_idx;
        let canvas_end = end_idx;
        let canvas_candles_per_screen = candles_per_screen;

        // Clone values for crosshair overlay
        let chart_bounds_for_crosshair = chart_bounds_shared.clone();
        let quotes_for_crosshair = quotes.clone();

        // Clone values for price axis
        let chart_bounds_for_price = chart_bounds_shared.clone();

        // Clone values for time axis
        let chart_bounds_for_time = chart_bounds_shared.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x000000))
            .child(header)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .m_2()
                    .child({
                        let mut chart_container = div()
                            .flex_1()
                            .relative()
                            .bg(rgb(0x000000))
                            .rounded_md()
                            .child(
                                canvas(
                                    move |bounds: Bounds<Pixels>, _window, _cx| {
                                        // Store bounds for crosshair calculations
                                        if let Ok(mut guard) = chart_bounds_for_canvas.write() {
                                            *guard = Some(bounds);
                                        }

                                        let visible = &quotes_for_canvas[canvas_start..canvas_end];
                                        let count = visible.len();
                                        if count == 0 || bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
                                            return ChartPaintData {
                                                candles: Vec::new(),
                                                volume_bars: Vec::new(),
                                            };
                                        }

                                        let (body_width, wick_width) =
                                            candle_dimensions(canvas_candles_per_screen);

                                        let price_range = max_price - min_price;
                                        let chart_w: f64 = bounds.size.width.into();
                                        let chart_h: f64 = bounds.size.height.into();
                                        let candle_slot_w = chart_w / count as f64;

                                        // Compute candle geometries
                                        let mut candles = Vec::with_capacity(count);
                                        for (i, q) in visible.iter().enumerate() {
                                            let is_bullish = q.close >= q.open;
                                            let cx_mid = candle_slot_w * (i as f64 + 0.5);

                                            let (wick_bounds, body_bounds) = if price_range == 0.0 {
                                                // Flat-line: draw candle as horizontal line at center
                                                let center_y = chart_h / 2.0;
                                                (
                                                    Bounds {
                                                        origin: point(
                                                            bounds.origin.x + px(cx_mid as f32 - wick_width / 2.0),
                                                            bounds.origin.y + px(center_y as f32 - 0.5),
                                                        ),
                                                        size: size(px(wick_width), px(1.0)),
                                                    },
                                                    Bounds {
                                                        origin: point(
                                                            bounds.origin.x + px(cx_mid as f32 - body_width / 2.0),
                                                            bounds.origin.y + px(center_y as f32 - 1.5),
                                                        ),
                                                        size: size(px(body_width), px(3.0)),
                                                    },
                                                )
                                            } else {
                                                // Wick
                                                let wick_top_y = (max_price - q.high.into_inner()) / price_range * chart_h;
                                                let wick_bot_y = (max_price - q.low.into_inner()) / price_range * chart_h;
                                                let wick_h = (wick_bot_y - wick_top_y).max(0.0);

                                                let wb = Bounds {
                                                    origin: point(
                                                        bounds.origin.x + px(cx_mid as f32 - wick_width / 2.0),
                                                        bounds.origin.y + px(wick_top_y as f32),
                                                    ),
                                                    size: size(px(wick_width), px(wick_h as f32)),
                                                };

                                                // Body
                                                let open_y = (max_price - q.open.into_inner()) / price_range * chart_h;
                                                let close_y = (max_price - q.close.into_inner()) / price_range * chart_h;
                                                let body_top_y = open_y.min(close_y);
                                                let body_h = (open_y - close_y).abs().max(1.0);

                                                let bb = Bounds {
                                                    origin: point(
                                                        bounds.origin.x + px(cx_mid as f32 - body_width / 2.0),
                                                        bounds.origin.y + px(body_top_y as f32),
                                                    ),
                                                    size: size(px(body_width), px(body_h as f32)),
                                                };

                                                (wb, bb)
                                            };

                                            candles.push(CandleGeometry {
                                                wick_bounds,
                                                body_bounds,
                                                is_bullish,
                                            });
                                        }

                                        // Compute volume bar geometries
                                        let max_volume = visible
                                            .iter()
                                            .map(|q| q.volume.into_inner())
                                            .fold(0.0_f64, |a, b| a.max(b));

                                        let mut volume_bars = Vec::with_capacity(count);
                                        if max_volume > 0.0 {
                                            let vol_area_h = chart_h * 0.2;
                                            let vol_area_top_y = chart_h - vol_area_h;

                                            for (i, q) in visible.iter().enumerate() {
                                                let vol = q.volume.into_inner();
                                                let bar_h = vol / max_volume * vol_area_h;
                                                let bar_x = candle_slot_w * i as f64;
                                                let bar_y = vol_area_top_y + (vol_area_h - bar_h);

                                                let bar_bounds = Bounds {
                                                    origin: point(
                                                        bounds.origin.x + px(bar_x as f32),
                                                        bounds.origin.y + px(bar_y as f32),
                                                    ),
                                                    size: size(
                                                        px(candle_slot_w as f32),
                                                        px(bar_h as f32),
                                                    ),
                                                };

                                                volume_bars.push(VolumeBarGeometry {
                                                    bounds: bar_bounds,
                                                    is_bullish: q.close >= q.open,
                                                });
                                            }
                                        }

                                        ChartPaintData {
                                            candles,
                                            volume_bars,
                                        }
                                    },
                                    move |bounds, data: ChartPaintData, window, _cx| {
                                        window.paint_layer(bounds, |window| {
                                            // Volume bars (behind candles)
                                            for bar in &data.volume_bars {
                                                let color = if bar.is_bullish {
                                                    rgba(0x4ade8030)
                                                } else {
                                                    rgba(0xf8717130)
                                                };
                                                window.paint_quad(fill(bar.bounds, color));
                                            }

                                            // Wicks
                                            for c in &data.candles {
                                                let color = if c.is_bullish {
                                                    rgb(0x4ade80)
                                                } else {
                                                    rgb(0xf87171)
                                                };
                                                window.paint_quad(fill(c.wick_bounds, color));
                                            }

                                            // Bodies
                                            for c in &data.candles {
                                                if c.is_bullish {
                                                    // Hollow body: background + green border
                                                    window.paint_quad(quad(
                                                        c.body_bounds,
                                                        0.,
                                                        rgb(0x0a0a0a),
                                                        px(1.),
                                                        rgb(0x4ade80),
                                                        BorderStyle::default(),
                                                    ));
                                                } else {
                                                    // Solid red body
                                                    window.paint_quad(fill(
                                                        c.body_bounds,
                                                        rgb(0xf87171),
                                                    ));
                                                }
                                            }
                                        });
                                    },
                                )
                                .absolute()
                                .size_full(),
                            );

                        if let (Some(pos), Some(idx)) = (crosshair_position, hovered_quote_index) {
                            if let Some(quote) = quotes_for_crosshair.get(idx) {
                                let cb = chart_bounds_for_crosshair
                                    .read()
                                    .ok()
                                    .and_then(|g| *g);
                                chart_container = chart_container
                                    .child(render_crosshair_overlay(quote, pos, cb));
                            }
                        }

                        chart_container
                    })
                    .child({
                        let cursor_price = if let Some(pos) = crosshair_position {
                            if let Some(bounds) =
                                chart_bounds_for_price.read().ok().and_then(|g| *g)
                            {
                                let adj_y = pos.y - bounds.origin.y;
                                let price_range = max_price - min_price;
                                let y_ratio: f64 = adj_y.into();
                                let height: f64 = bounds.size.height.into();
                                Some(max_price - ((y_ratio / height) * price_range))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        render_price_axis(min_price, max_price, cursor_price)
                    }),
            )
            .child({
                let (cursor_time, cursor_x_pct) = if let (Some(pos), Some(bounds)) = (
                    crosshair_position,
                    chart_bounds_for_time.read().ok().and_then(|g| *g),
                ) {
                    let adj_x: f64 = (pos.x - bounds.origin.x).into();
                    let width: f64 = bounds.size.width.into();
                    let x_ratio = adj_x / width;
                    let candle_idx = ((x_ratio * display_quotes.len() as f64) as usize)
                        .min(display_quotes.len().saturating_sub(1));

                    if let Some(quote) = display_quotes.get(candle_idx) {
                        let x_pct =
                            (candle_idx as f32 + 0.5) / display_quotes.len() as f32 * 100.0;
                        (Some(quote.time), Some(x_pct))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };
                render_time_axis(display_quotes, cursor_time, cursor_x_pct)
            })
            .into_any_element()
    }
}
