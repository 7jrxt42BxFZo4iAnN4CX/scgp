use std::sync::{Arc, RwLock};

use gpui::{prelude::*, *};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::chart::CandlestickChart;
use crate::data::{self, Quote};

fn help_row(key: &str, desc: &str) -> Div {
    div()
        .flex()
        .flex_row()
        .gap_3()
        .child(
            div()
                .w(px(70.))
                .text_sm()
                .text_color(rgb(0xe0e0e0))
                .font_weight(gpui::FontWeight::BOLD)
                .child(key.to_string()),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x808080))
                .child(desc.to_string()),
        )
}

pub struct ChartWindow {
    ticker: String,
    quotes: Arc<Vec<Quote>>,
    loading: bool,
    error_message: Option<String>,
    chart_scroll_offset: usize,
    chart_candles_per_screen: usize,
    is_dragging: bool,
    last_mouse_position: Option<Point<Pixels>>,
    crosshair_position: Option<Point<Pixels>>,
    hovered_quote_index: Option<usize>,
    chart_bounds: Arc<RwLock<Option<Bounds<Pixels>>>>,
    mouse_x_ratio: Option<f64>,
    drag_accumulator: f64,
    tokio_handle: tokio::runtime::Handle,
    interval: String,
    range: String,
    show_ticker_input: bool,
    show_help: bool,
    ticker_input: Entity<InputState>,
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

impl ChartWindow {
    pub fn new(
        ticker: String,
        interval: String,
        range: String,
        tokio_handle: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        let ticker_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Ticker (e.g. AAPL)")
        });

        // Subscribe to input Enter event (must store Subscription to keep it alive)
        let subscription = cx.subscribe_in(&ticker_input, window, |view: &mut Self, _, event: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                let new_ticker = view.ticker_input.read(cx).value().to_string().trim().to_uppercase();
                if !new_ticker.is_empty() {
                    view.show_ticker_input = false;
                    view.load_ticker(new_ticker, window, cx);
                }
            }
        });

        let this = Self {
            ticker: ticker.clone(),
            quotes: Arc::new(Vec::new()),
            loading: true,
            error_message: None,
            chart_scroll_offset: usize::MAX,
            chart_candles_per_screen: 200,
            is_dragging: false,
            last_mouse_position: None,
            crosshair_position: None,
            hovered_quote_index: None,
            chart_bounds: Arc::new(RwLock::new(None)),
            mouse_x_ratio: None,
            drag_accumulator: 0.0,
            tokio_handle: tokio_handle.clone(),
            interval: interval.clone(),
            range: range.clone(),
            show_ticker_input: false,
            show_help: false,
            ticker_input,
            focus_handle,
            _subscription: subscription,
        };

        // Start initial data loading
        Self::spawn_fetch(ticker, interval, range, tokio_handle, cx);

        this
    }

    fn load_ticker(&mut self, ticker: String, window: &mut Window, cx: &mut Context<Self>) {
        tracing::info!(ticker, "Switching ticker");
        self.ticker = ticker.clone();
        self.quotes = Arc::new(Vec::new());
        self.loading = true;
        self.error_message = None;
        self.chart_scroll_offset = usize::MAX;
        self.hovered_quote_index = None;
        self.crosshair_position = None;

        // Clear input text and refocus root
        self.ticker_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.focus_handle.focus(window);

        window.set_window_title(&format!("scgp - {}", ticker));

        let handle = self.tokio_handle.clone();
        let interval = self.interval.clone();
        let range = self.range.clone();

        Self::spawn_fetch(ticker, interval, range, handle, cx);

        cx.notify();
    }

    fn max_scroll_offset(&self) -> usize {
        self.quotes.len().saturating_sub(self.chart_candles_per_screen)
    }

    fn clamp_scroll_offset(&mut self) {
        let max = self.max_scroll_offset();
        if self.chart_scroll_offset > max {
            self.chart_scroll_offset = max;
        }
    }

    fn spawn_fetch(
        ticker: String,
        interval: String,
        range: String,
        handle: tokio::runtime::Handle,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |entity, cx| {
            let result = handle
                .spawn(async move { data::fetch_quotes(&ticker, &interval, &range).await })
                .await;

            if let Err(e) = entity.update(cx, |view: &mut Self, cx| match result {
                Ok(Ok(quotes)) => {
                    let max_offset = quotes.len().saturating_sub(view.chart_candles_per_screen);
                    view.chart_scroll_offset = max_offset;
                    view.quotes = Arc::new(quotes);
                    view.loading = false;
                    cx.notify();
                }
                Ok(Err(e)) => {
                    tracing::error!(error = %e, "Fetch failed");
                    view.error_message = Some(e.to_string());
                    view.loading = false;
                    cx.notify();
                }
                Err(e) => {
                    tracing::error!(error = %e, "Fetch task panicked");
                    view.error_message = Some(format!("Task error: {}", e));
                    view.loading = false;
                    cx.notify();
                }
            }) {
                tracing::warn!("Failed to update ChartWindow entity: {}", e);
            }
        })
        .detach();
    }
}

impl Render for ChartWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.loading {
            return div()
                .size_full()
                .bg(rgb(0x0a0a0a))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_lg()
                        .text_color(rgb(0xa0a0a0))
                        .child(format!("Loading {}...", self.ticker)),
                )
                .into_any_element();
        }

        if let Some(ref err) = self.error_message {
            return div()
                .size_full()
                .bg(rgb(0x0a0a0a))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .track_focus(&self.focus_handle)
                .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                    match event.keystroke.key.as_str() {
                        "enter" => {
                            let ticker = view.ticker.clone();
                            view.load_ticker(ticker, window, cx);
                        }
                        "escape" => {
                            view.error_message = None;
                            view.loading = false;
                            view.show_ticker_input = true;
                            view.ticker_input.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                            cx.notify();
                        }
                        _ => {}
                    }
                }))
                .child(
                    div()
                        .text_lg()
                        .text_color(rgb(0xf87171))
                        .child(err.clone()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x707070))
                        .child("Press Enter to retry  |  Press Escape to change ticker"),
                )
                .into_any_element();
        }

        // Clamp scroll offset (write back to keep state canonical)
        self.clamp_scroll_offset();
        let quotes = self.quotes.clone();
        let scroll_offset = self.chart_scroll_offset;
        let candles_per_screen = self.chart_candles_per_screen;
        let show_input = self.show_ticker_input;
        let show_help = self.show_help;

        if !self.focus_handle.is_focused(window) && !self.show_ticker_input {
            self.focus_handle.focus(window);
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x0a0a0a))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => {
                        if view.show_help {
                            view.show_help = false;
                        } else if view.show_ticker_input {
                            view.show_ticker_input = false;
                            view.focus_handle.focus(window);
                        } else {
                            view.show_ticker_input = true;
                            view.ticker_input.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                        }
                        cx.notify();
                    }
                    "?" => {
                        view.show_help = !view.show_help;
                        cx.notify();
                    }
                    _ => {}
                }
            }))
            .child(
                // Chart area with mouse handlers
                div()
                    .flex_1()
                    .on_mouse_move(cx.listener(
                        |view, event: &gpui::MouseMoveEvent, _window, cx| {
                            view.crosshair_position = Some(event.position);

                            // Compute mouse_x_ratio for zoom-to-cursor
                            if let Ok(guard) = view.chart_bounds.read()
                                && let Some(bounds) = *guard {
                                    let relative_x: f64 = ((event.position.x
                                        - bounds.origin.x)
                                        / bounds.size.width)
                                        .into();
                                    view.mouse_x_ratio = Some(relative_x.clamp(0.0, 1.0));
                                }

                            // Drag panning
                            if view.is_dragging {
                                if let Some(last_pos) = view.last_mouse_position {
                                    let delta_x: f64 =
                                        (event.position.x - last_pos.x).into();

                                    if let Ok(guard) = view.chart_bounds.read()
                                        && let Some(bounds) = *guard {
                                            let chart_width: f64 =
                                                bounds.size.width.into();
                                            if chart_width > 0.0 {
                                                let candles_per_pixel =
                                                    view.chart_candles_per_screen as f64
                                                        / chart_width;
                                                view.drag_accumulator +=
                                                    delta_x * candles_per_pixel;
                                                let candle_delta =
                                                    view.drag_accumulator as isize;
                                                view.drag_accumulator -=
                                                    candle_delta as f64;

                                                let max_offset = view
                                                    .quotes
                                                    .len()
                                                    .saturating_sub(
                                                        view.chart_candles_per_screen,
                                                    );

                                                if candle_delta < 0 {
                                                    view.chart_scroll_offset = view
                                                        .chart_scroll_offset
                                                        .saturating_add(
                                                            (-candle_delta) as usize,
                                                        )
                                                        .min(max_offset);
                                                } else if candle_delta > 0 {
                                                    view.chart_scroll_offset = view
                                                        .chart_scroll_offset
                                                        .saturating_sub(
                                                            candle_delta as usize,
                                                        );
                                                }
                                            }
                                        }
                                }
                                view.last_mouse_position = Some(event.position);
                            }

                            // Compute hovered candle index
                            if let Ok(guard) = view.chart_bounds.read()
                                && let Some(bounds) = *guard {
                                    let relative_x: f64 = ((event.position.x
                                        - bounds.origin.x)
                                        / bounds.size.width)
                                        .into();
                                    let relative_x = relative_x.clamp(0.0, 1.0);

                                    let max_offset = view
                                        .quotes
                                        .len()
                                        .saturating_sub(view.chart_candles_per_screen);
                                    let clamped_offset =
                                        view.chart_scroll_offset.min(max_offset);
                                    let visible_count = view
                                        .chart_candles_per_screen
                                        .min(
                                            view.quotes
                                                .len()
                                                .saturating_sub(clamped_offset),
                                        );

                                    if visible_count > 0 {
                                        let candle_idx =
                                            (relative_x * visible_count as f64) as usize;
                                        let quote_idx = clamped_offset
                                            + candle_idx
                                                .min(visible_count.saturating_sub(1));
                                        view.hovered_quote_index = Some(quote_idx);
                                    }
                                }

                            cx.notify();
                        },
                    ))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(
                            |view, event: &gpui::MouseDownEvent, _window, cx| {
                                view.is_dragging = true;
                                view.drag_accumulator = 0.0;
                                view.last_mouse_position = Some(event.position);
                                cx.notify();
                            },
                        ),
                    )
                    .on_mouse_up(
                        gpui::MouseButton::Left,
                        cx.listener(|view, _event: &gpui::MouseUpEvent, _window, cx| {
                            view.is_dragging = false;
                            view.last_mouse_position = None;
                            cx.notify();
                        }),
                    )
                    .on_scroll_wheel(cx.listener(
                        |view, event: &gpui::ScrollWheelEvent, _window, cx| {
                            use gpui::ScrollDelta;
                            let delta_y = match event.delta {
                                ScrollDelta::Pixels(delta) => delta.y,
                                ScrollDelta::Lines(delta) => px(delta.y * 20.0),
                            };

                            let available_candles = view.quotes.len();

                            if available_candles == 0 {
                                return;
                            }

                            let mouse_ratio = view.mouse_x_ratio.unwrap_or(0.5);

                            let max_offset = available_candles
                                .saturating_sub(view.chart_candles_per_screen);
                            let clamped_offset =
                                view.chart_scroll_offset.min(max_offset);

                            let candle_under_cursor = clamped_offset
                                + (view.chart_candles_per_screen as f64 * mouse_ratio)
                                    as usize;

                            let zoom_step =
                                (view.chart_candles_per_screen / 7).max(20);

                            if delta_y > px(0.0) {
                                if view.chart_candles_per_screen > 50 {
                                    view.chart_candles_per_screen = view
                                        .chart_candles_per_screen
                                        .saturating_sub(zoom_step)
                                        .max(50);
                                } else {
                                    return;
                                }
                            } else if delta_y < px(0.0) {
                                if view.chart_candles_per_screen < available_candles {
                                    view.chart_candles_per_screen =
                                        (view.chart_candles_per_screen + zoom_step)
                                            .min(available_candles);
                                } else {
                                    return;
                                }
                            }

                            let new_offset_from_cursor =
                                (view.chart_candles_per_screen as f64 * mouse_ratio)
                                    as usize;
                            view.chart_scroll_offset = candle_under_cursor
                                .saturating_sub(new_offset_from_cursor)
                                .min(
                                    available_candles
                                        .saturating_sub(view.chart_candles_per_screen),
                                );

                            cx.notify();
                        },
                    ))
                    .child(CandlestickChart::new(
                        quotes.clone(),
                        scroll_offset,
                        candles_per_screen,
                        self.crosshair_position,
                        self.hovered_quote_index,
                        self.chart_bounds.clone(),
                        self.ticker.clone(),
                        self.interval.clone(),
                        self.range.clone(),
                    )),
            )
            // Ticker input overlay
            .when(show_input, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(px(0.))
                        .left(px(0.))
                        .right(px(0.))
                        .bottom(px(0.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgba(0x00000088))
                        .child(
                            div()
                                .w(px(300.))
                                .p_4()
                                .bg(rgb(0x1a1a1a))
                                .border_1()
                                .border_color(rgb(0x3e3e3e))
                                .rounded_lg()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(0xa0a0a0))
                                        .child("Enter ticker symbol:"),
                                )
                                .child(Input::new(&self.ticker_input)),
                        ),
                )
            })
            // Help overlay
            .when(show_help, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(px(0.))
                        .left(px(0.))
                        .right(px(0.))
                        .bottom(px(0.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgba(0x00000088))
                        .child(
                            div()
                                .w(px(280.))
                                .p_4()
                                .bg(rgb(0x1a1a1a))
                                .border_1()
                                .border_color(rgb(0x3e3e3e))
                                .rounded_lg()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_base()
                                        .text_color(rgb(0xffffff))
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .child("Keyboard Shortcuts"),
                                )
                                .child(help_row("Escape", "Change ticker"))
                                .child(help_row("Scroll", "Zoom in/out"))
                                .child(help_row("Drag", "Pan chart"))
                                .child(help_row("Enter", "Retry (on error)"))
                                .child(help_row("?", "Toggle help")),
                        ),
                )
            })
            .into_any_element()
    }
}
