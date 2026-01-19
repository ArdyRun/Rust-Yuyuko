// Visualization utilities for stat command
// Uses charts-rs library for professional quality charts

use chrono::{Datelike, NaiveDate, Duration, Utc};
use image::{Rgba, RgbaImage, ImageBuffer, ImageEncoder};
use std::collections::HashMap;
use ab_glyph::{FontRef, PxScale};
use imageproc::drawing::draw_text_mut;
use charts_rs::{BarChart, HeatmapChart, Box as ChartBox, svg_to_png, Series, THEME_DARK};

// Embed BOLD font at compile time for heatmap (charts-rs handles its own fonts)
const FONT_DATA: &[u8] = include_bytes!("../assets/NotoSansJP-Bold.ttf");

// Colors for heatmap (keeping manual implementation for now since charts-rs heatmap is different format)
const BG_COLOR: Rgba<u8> = Rgba([30, 30, 32, 255]);
const CELL_EMPTY: Rgba<u8> = Rgba([45, 45, 48, 255]);
const CELL_L1: Rgba<u8> = Rgba([0, 100, 50, 255]);
const CELL_L2: Rgba<u8> = Rgba([0, 150, 70, 255]);
const CELL_L3: Rgba<u8> = Rgba([50, 200, 100, 255]);
const CELL_L4: Rgba<u8> = Rgba([100, 255, 130, 255]);
const LABEL_COLOR: Rgba<u8> = Rgba([255, 255, 255, 255]);
const GRAY_COLOR: Rgba<u8> = Rgba([150, 150, 150, 255]);
const TODAY_BORDER: Rgba<u8> = Rgba([255, 255, 255, 255]);

// Month labels in Japanese
const MONTHS: [&str; 12] = ["1月", "2月", "3月", "4月", "5月", "6月", 
                            "7月", "8月", "9月", "10月", "11月", "12月"];

// Day labels in Japanese kanji
const DAYS: [&str; 7] = ["日", "月", "火", "水", "木", "金", "土"];

// Legend colors (from less to more)
const LEGEND_COLORS: [Rgba<u8>; 6] = [CELL_EMPTY, CELL_L1, CELL_L2, CELL_L3, CELL_L4, Rgba([130, 255, 160, 255])];

/// Activity level thresholds (points)
fn get_activity_color(points: i64, max_points: i64) -> Rgba<u8> {
    if points == 0 {
        return CELL_EMPTY;
    }
    let ratio = points as f64 / max_points.max(1) as f64;
    if ratio > 0.8 {
        Rgba([130, 255, 160, 255])
    } else if ratio > 0.6 {
        CELL_L4
    } else if ratio > 0.4 {
        CELL_L3
    } else if ratio > 0.2 {
        CELL_L2
    } else {
        CELL_L1
    }
}

/// Generate a GitHub-style heatmap image for user activity
/// Returns PNG bytes
pub fn generate_heatmap(
    daily_points: &HashMap<String, i64>,
    year: i32,
    _username: &str,
) -> Result<Vec<u8>, String> {
    // Keep manual implementation for GitHub-style heatmap (charts-rs heatmap is matrix-style)
    const CELL_SIZE: u32 = 14;
    const GAP: u32 = 3;
    const COLS: u32 = 53;
    const ROWS: u32 = 7;
    const PADDING_LEFT: u32 = 40;
    const PADDING_TOP: u32 = 65;
    const PADDING_RIGHT: u32 = 30;
    const PADDING_BOTTOM: u32 = 75;
    
    let width = COLS * (CELL_SIZE + GAP) + PADDING_LEFT + PADDING_RIGHT;
    let height = ROWS * (CELL_SIZE + GAP) + PADDING_TOP + PADDING_BOTTOM;
    
    let mut img: RgbaImage = ImageBuffer::from_pixel(width, height, BG_COLOR);
    
    let font = FontRef::try_from_slice(FONT_DATA)
        .map_err(|e| format!("Failed to load font: {:?}", e))?;
    
    let max_points = daily_points.values().copied().max().unwrap_or(1);
    let days_active = daily_points.values().filter(|&&p| p > 0).count();
    let total_points: i64 = daily_points.values().sum();
    let avg_points = if days_active > 0 { 
        total_points as f64 / days_active as f64 
    } else { 
        0.0 
    };
    
    let today = Utc::now().format("%Y-%m-%d").to_string();
    
    // Draw title
    let title = format!("Immersion Heatmap - {}", year);
    let title_scale = PxScale::from(18.0);
    draw_text_mut(&mut img, LABEL_COLOR, 15, 12, title_scale, &font, &title);
    
    let start_date = NaiveDate::from_ymd_opt(year, 1, 1).ok_or("Invalid year")?;
    let end_date = NaiveDate::from_ymd_opt(year, 12, 31).ok_or("Invalid year")?;
    let days_since_sunday = start_date.weekday().num_days_from_sunday();
    let grid_start = start_date - Duration::days(days_since_sunday as i64);
    
    let mut month_cols: [Option<u32>; 12] = [None; 12];
    let mut current_date = grid_start;
    let mut col = 0;
    let mut row;
    
    while current_date <= end_date && col < COLS {
        row = current_date.weekday().num_days_from_sunday() as u32;
        
        if current_date.year() == year {
            let month_idx = (current_date.month() - 1) as usize;
            if month_cols[month_idx].is_none() {
                month_cols[month_idx] = Some(col);
            }
        }
        
        if current_date.year() == year || (current_date < start_date && col == 0) {
            let date_str = current_date.format("%Y-%m-%d").to_string();
            let points = daily_points.get(&date_str).copied().unwrap_or(0);
            let color = get_activity_color(points, max_points);
            
            let x = PADDING_LEFT + col * (CELL_SIZE + GAP);
            let y = PADDING_TOP + row * (CELL_SIZE + GAP);
            let is_today = date_str == today;
            
            if is_today {
                for dx in 0..CELL_SIZE + 2 {
                    for dy in 0..CELL_SIZE + 2 {
                        let px = x.saturating_sub(1) + dx;
                        let py = y.saturating_sub(1) + dy;
                        if px < width && py < height {
                            img.put_pixel(px, py, TODAY_BORDER);
                        }
                    }
                }
            }
            
            for dx in 0..CELL_SIZE {
                for dy in 0..CELL_SIZE {
                    if x + dx < width && y + dy < height {
                        img.put_pixel(x + dx, y + dy, color);
                    }
                }
            }
        }
        
        current_date = current_date + Duration::days(1);
        if current_date.weekday().num_days_from_sunday() == 0 {
            col += 1;
        }
    }
    
    // Draw month labels
    let month_scale = PxScale::from(13.0);
    for (month_idx, maybe_col) in month_cols.iter().enumerate() {
        if let Some(col) = maybe_col {
            let x = PADDING_LEFT + col * (CELL_SIZE + GAP);
            draw_text_mut(&mut img, LABEL_COLOR, x as i32, 42, month_scale, &font, MONTHS[month_idx]);
        }
    }
    
    // Draw day labels
    let day_scale = PxScale::from(14.0);
    for (row, day_name) in DAYS.iter().enumerate() {
        let y = PADDING_TOP + (row as u32) * (CELL_SIZE + GAP);
        draw_text_mut(&mut img, GRAY_COLOR, 20, y as i32, day_scale, &font, day_name);
    }
    
    // Draw legend
    let legend_y = height - 35;
    let legend_x = PADDING_LEFT;
    let legend_scale = PxScale::from(12.0);
    draw_text_mut(&mut img, GRAY_COLOR, (legend_x - 5) as i32, legend_y as i32, legend_scale, &font, "Less");
    for (i, color) in LEGEND_COLORS.iter().enumerate() {
        let box_x = legend_x + 35 + (i as u32) * 18;
        for dx in 0..14 { for dy in 0..14 { img.put_pixel(box_x + dx, legend_y + dy, *color); } }
    }
    draw_text_mut(&mut img, GRAY_COLOR, (legend_x + 35 + 6 * 18 + 5) as i32, legend_y as i32, legend_scale, &font, "More");
    
    // Draw stats
    let heatmap_right_edge = PADDING_LEFT + COLS * (CELL_SIZE + GAP);
    let stats_x = heatmap_right_edge - 150;
    let stats_y_base = height - 30;
    let stats_scale = PxScale::from(12.0);
    draw_text_mut(&mut img, GRAY_COLOR, stats_x as i32, (stats_y_base - 30) as i32, stats_scale, &font, &format!("{} days active", days_active));
    draw_text_mut(&mut img, GRAY_COLOR, stats_x as i32, (stats_y_base - 15) as i32, stats_scale, &font, &format!("{} total points", total_points));
    draw_text_mut(&mut img, GRAY_COLOR, stats_x as i32, stats_y_base as i32, stats_scale, &font, &format!("{:.1} avg points/day", avg_points));
    
    // Encode to PNG
    let mut png_bytes: Vec<u8> = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        encoder.write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgba8)
            .map_err(|e| format!("PNG encoding failed: {:?}", e))?;
    }
    
    Ok(png_bytes)
}

/// Bar chart data point
pub struct BarData {
    pub label: String,
    pub value: f64,
    pub media_type: String,
}

/// Generate a professional bar chart using charts-rs
/// Returns PNG bytes
pub fn generate_bar_chart(
    data: &[BarData],
    title: &str,
    _value_label: &str,
) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("No data to chart".to_string());
    }
    
    // Prepare data for charts-rs
    let values: Vec<f32> = data.iter().map(|d| d.value as f32).collect();
    let labels: Vec<String> = data.iter().map(|d| d.label.clone()).collect();
    
    // Create series
    let series_data: Vec<(String, Vec<f32>)> = data.iter()
        .map(|d| (d.label.clone(), vec![d.value as f32]))
        .collect();
    
    // Create bar chart with dark theme
    let mut bar_chart = BarChart::new_with_theme(
        series_data.into_iter().map(|(name, values)| (name.as_str(), values).into()).collect(),
        vec!["Points".to_string()],
        THEME_DARK,
    );
    
    // Configure chart
    bar_chart.width = 800.0;
    bar_chart.height = 450.0;
    bar_chart.title_text = title.to_string();
    bar_chart.title_font_size = 24.0;
    bar_chart.legend_show = Some(true);
    bar_chart.legend_margin = Some(ChartBox {
        top: 50.0,
        bottom: 10.0,
        left: 10.0,
        right: 10.0,
    });
    
    // Generate SVG then convert to PNG
    let svg = bar_chart.svg()
        .map_err(|e| format!("SVG generation failed: {:?}", e))?;
    
    let png_data = svg_to_png(&svg)
        .map_err(|e| format!("PNG conversion failed: {:?}", e))?;
    
    Ok(png_data)
}
