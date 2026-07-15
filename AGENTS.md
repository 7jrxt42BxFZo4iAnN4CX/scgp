# AGENTS.md — Rust/scgp

## Обзор проекта

**Название:** scgp — Standalone GPU-accelerated Candlestick Chart Viewer  
**Язык:** Rust (edition 2024)  
**Тип:** Desktop GUI приложение (GPUI + canvas)  
**Статус:** Рабочий, публичный проект (GitHub)  
**Сложность:** Средняя-высокая (~667 строк в chart.rs, ~682 в app.rs)

## Архитектура

### Модули

| Модуль | Строк | Назначение |
|--------|-------|------------|
| `main.rs` | 136 | CLI парсинг, инициализация GPUI, создание окна |
| `app.rs` | 682 | ChartWindow: состояние, обработка ввода, загрузка данных |
| `chart.rs` | 667 | CandlestickChart: рендеринг свечей, осей, crosshair |
| `data.rs` | 128 | Quote модель, Yahoo Finance клиент, классификация ошибок |

### Зависимости

| Крейт | Назначение |
|--------|------------|
| `gpui` | GPUI фреймворк (Zed) |
| `gpui-component` | Компоненты GPUI (Input, Root) |
| `gpui-component-assets` | Ассеты |
| `yahoo_finance_api` | Yahoo Finance API |
| `tokio` | Async runtime |
| `ordered-float` | NotNan f64 |
| `chrono`/`time` | Дата/время |
| `clap` | CLI парсинг |
| `anyhow` | Error handling |
| `tracing` | Логирование |

## UI Layout

```
┌─────────────────────────────────────────────────────┐
│ AAPL · 1d · 1y              (header)               │
├─────────────────────────────────────────────────────┤
│ ┌─────────────────────────────────────────────┬───┐ │
│ │                                             │$185│ │
│ │              📈 CANVAS                      │$180│ │
│ │              (свечи + volume bars)          │$175│ │
│ │                                             │$170│ │
│ │  ┌──────────────┐                           │$165│ │
│ │  │ OHLCV Info   │                           │    │ │
│ │  └──────────────┘                           │    │ │
│ └─────────────────────────────────────────────┴───┘ │
│ 2024-01  2024-04  2024-07  2024-10  2025-01        │
└─────────────────────────────────────────────────────┘
```

## Ключевые особенности

### 1. Canvas-based рендеринг
- Прямой рендеринг через `gpui::canvas()` (не plotters!)
- Hardware-accelerated через GPUI
- Адаптивные размеры свечей (body_width, wick_width)
- Volume bars (20% высоты чарта)

### 2. Интерактивность
- **Scroll wheel:** Zoom с привязкой к курсору
- **Drag:** Панорамирование
- **Crosshair:** OHLCV + дата/время
- **Escape:** Overlay для смены тикера

### 3. CLI аргументы

```bash
scgp -t AAPL                        # Daily 1y
scgp -t MSFT -i 1h -r 730d         # Hourly ~2 года
scgp -t TSLA -i 5m -r 60d -s 2560x1440
scgp -t BTC-USD -i 1d -r max -m    # Full history, maximized
```

| Флаг | Описание | По умолчанию |
|------|----------|--------------|
| `-t` | Тикер | (пусто → overlay) |
| `-i` | Интервал | 1d |
| `-r` | Диапазон | 1y |
| `-s` | Размер окна | 1920x1080 |
| `-m` | Maximized | false |

### 4. Тёмная тема
- Фон: `rgb(0x0a0a0a)`
- Bullish свечи: `rgb(0x4ade80)` (пустотелые)
- Bearish свечи: `rgb(0xf87171)` (залитые)
- Crosshair: полупрозрачные линии

### 5. Оси
- Price axis: 8 ценовых меток + cursor tracking
- Time axis: адаптивный формат (%Y-%m, %Y-%m-%d, %m/%d %H:%M)
- Cursor labels: синий фон + белый текст

## Yahoo Finance Диапазоны

| Интервал | Макс. диапазон |
|----------|----------------|
| 1m | 7 дней |
| 5m, 15m, 30m | 60 дней |
| 1h | 730 дней |
| 1d, 1wk, 1mo | unlimited (max) |

## Инварианты

- GPUI обязателен (не egui!)
- `yahoo_finance_api` — стандартная версия (не кастомная)
- Data fetch в `tokio::spawn`, результат через entity update
- Candle dimensions: адаптивные пороги (50/100/200/350 свечей)
- NotNan<f64> для безопасных вычислений цен
- `Box::leak` для tokio runtime (lifetime = process)
