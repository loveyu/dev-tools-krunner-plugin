//! 应用图标的跨平台像素数据。

pub const ICON_SIZE: u32 = 64;

/// 生成带圆角背景和代码括号的 RGBA 图标。
pub fn rgba() -> Vec<u8> {
    let mut pixels = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            if !inside_rounded_square(x, y) {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }

            let progress = (x + y) as f32 / (2 * (ICON_SIZE - 1)) as f32;
            let red = interpolate(37, 124, progress);
            let green = interpolate(99, 58, progress);
            let blue = interpolate(235, 237, progress);
            let foreground = is_code_mark(x, y);
            let pixel = if foreground {
                [255, 255, 255, 255]
            } else {
                [red, green, blue, 255]
            };
            pixels.extend_from_slice(&pixel);
        }
    }
    pixels
}

/// 创建供 tao 主窗口使用的图标。
pub fn window_icon() -> Result<tao::window::Icon, tao::window::BadIcon> {
    tao::window::Icon::from_rgba(rgba(), ICON_SIZE, ICON_SIZE)
}

fn interpolate(start: u8, end: u8, progress: f32) -> u8 {
    (f32::from(start) + (f32::from(end) - f32::from(start)) * progress).round() as u8
}

fn inside_rounded_square(x: u32, y: u32) -> bool {
    const EDGE: i32 = 4;
    const RADIUS: i32 = 12;
    let x = x as i32;
    let y = y as i32;
    if !(EDGE..64 - EDGE).contains(&x) || !(EDGE..64 - EDGE).contains(&y) {
        return false;
    }
    let center_x = x.clamp(EDGE + RADIUS, 63 - EDGE - RADIUS);
    let center_y = y.clamp(EDGE + RADIUS, 63 - EDGE - RADIUS);
    let delta_x = x - center_x;
    let delta_y = y - center_y;
    delta_x * delta_x + delta_y * delta_y <= RADIUS * RADIUS
}

fn is_code_mark(x: u32, y: u32) -> bool {
    let point = (f64::from(x), f64::from(y));
    let segments = [
        ((27.0, 19.0), (17.0, 32.0)),
        ((17.0, 32.0), (27.0, 45.0)),
        ((37.0, 19.0), (47.0, 32.0)),
        ((47.0, 32.0), (37.0, 45.0)),
        ((36.0, 17.0), (28.0, 47.0)),
    ];
    segments
        .into_iter()
        .any(|(start, end)| distance_to_segment(point, start, end) <= 2.25)
}

fn distance_to_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let segment = (end.0 - start.0, end.1 - start.1);
    let offset = (point.0 - start.0, point.1 - start.1);
    let length_squared = segment.0 * segment.0 + segment.1 * segment.1;
    let projection =
        ((offset.0 * segment.0 + offset.1 * segment.1) / length_squared).clamp(0.0, 1.0);
    let closest = (
        start.0 + projection * segment.0,
        start.1 + projection * segment.1,
    );
    (point.0 - closest.0).hypot(point.1 - closest.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_expected_dimensions_and_alpha_edges() {
        let pixels = rgba();
        assert_eq!(pixels.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        assert_eq!(&pixels[..4], &[0, 0, 0, 0]);

        let center = ((32 * ICON_SIZE + 32) * 4) as usize;
        assert_eq!(pixels[center + 3], 255);
    }

    #[test]
    fn icon_contains_white_code_marks() {
        let pixels = rgba();
        let center = ((32 * ICON_SIZE + 32) * 4) as usize;
        assert_eq!(&pixels[center..center + 4], &[255, 255, 255, 255]);
    }
}
