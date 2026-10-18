use super::sampling_quad;
use eframe::egui;
#[test]
fn filtered_quad_coverage_matches_independent_sdl_opengl_frames() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/sdl-filtered-edges.json")).unwrap();
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0));
    for case in reference["cases"].as_array().unwrap() {
        let scale = case["scale"].as_f64().unwrap() as f32;
        let rect = &case["rect"];
        let bounds = std::array::from_fn(|i| rect[i].as_i64().unwrap() as f32 * scale);
        let (coverage, uv) = sampling_quad(bounds, viewport, 1.0);
        assert!(coverage.iter().chain(&uv).all(|v| v.is_finite()));
        let left = ((coverage[0] + 1.0) * 320.0).round();
        let top = ((1.0 - coverage[1]) * 240.0).round();
        let right = (left + coverage[2] * 320.0).round();
        let bottom = (top - coverage[3] * 240.0).round();
        let actual: Vec<_> = (0..64)
            .flat_map(|y| (0..64).map(move |x| [x, y]))
            .filter(|[x, y]| {
                (*x as f32 + 0.5) >= left
                    && (*x as f32 + 0.5) < right
                    && (*y as f32 + 0.5) >= top
                    && (*y as f32 + 0.5) < bottom
            })
            .collect();
        let expected: Vec<[u32; 2]> = serde_json::from_value(case["pixels"].clone()).unwrap();
        assert_eq!(actual, expected, "{case}");
    }
}
#[test]
fn translated_fractional_dpi_coverage_keeps_original_sampling_coordinates() {
    for ppp in [1.0, 1.25, 2.0] {
        let viewport = egui::Rect::from_min_size(egui::pos2(3.25, 5.5), egui::vec2(640.0, 480.0));
        let bounds = [10.5, 20.5, 2.0, 2.0];
        let (coverage, uv) = sampling_quad(bounds, viewport, ppp);
        let left = f64::from((viewport.left() * ppp).round());
        let top = f64::from((viewport.top() * ppp).round());
        let width = f64::from((viewport.right() * ppp).round()) - left;
        let height = f64::from((viewport.bottom() * ppp).round()) - top;
        let x = left + (f64::from(coverage[0]) + 1.0) * width / 2.0;
        let y = top + (1.0 - f64::from(coverage[1])) * height / 2.0;
        let original_x = left + 10.5 * width / 640.0;
        let original_y = top + 20.5 * height / 480.0;
        assert!((x - f64::from(uv[0]) * 2.0 * width / 640.0 - original_x).abs() < 0.0001);
        assert!((y - f64::from(uv[1]) * 2.0 * height / 480.0 - original_y).abs() < 0.0001);
    }
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0));
    for bounds in [
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, f32::MIN_POSITIVE, 1.0],
    ] {
        let (coverage, uv) = sampling_quad(bounds, viewport, 1.0);
        assert!(coverage.iter().chain(&uv).all(|v| v.is_finite()));
        assert_eq!(coverage[2] * coverage[3], 0.0);
    }
}

#[test]
fn solid_rectangles_match_independent_sdl_pixel_centers() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/sdl-rectangle-edges.json"
    ))
    .unwrap();
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0));
    for case in reference["cases"].as_array().unwrap() {
        let scale = case["scale"].as_f64().unwrap() as f32;
        let bounds = std::array::from_fn(|i| case["rect"][i].as_i64().unwrap() as f32 * scale);
        let rectangle = super::solid_rectangle(bounds, viewport, 1.0);
        let actual: Vec<_> = (0..64)
            .flat_map(|y| (0..64).map(move |x| [x, y]))
            .filter(|[x, y]| rectangle.contains(egui::pos2(*x as f32 + 0.5, *y as f32 + 0.5)))
            .collect();
        let expected: Vec<[u32; 2]> = serde_json::from_value(case["pixels"].clone()).unwrap();
        assert_eq!(actual, expected, "{case}");
    }
    for ppp in [1.0, 1.25, 2.0] {
        let viewport = egui::Rect::from_min_size(egui::pos2(3.25, 5.5), egui::vec2(640.0, 480.0));
        let bounds = [10.5, 20.5, 2.0, 2.0];
        let covered = super::solid_rectangle(bounds, viewport, ppp);
        let width = (viewport.right() * ppp).round() - (viewport.left() * ppp).round();
        let height = (viewport.bottom() * ppp).round() - (viewport.top() * ppp).round();
        let x = (viewport.left() * ppp).round() + bounds[0] * width / 640.0;
        let y = (viewport.top() * ppp).round() + bounds[1] * height / 480.0;
        assert!((covered.left() * ppp - (x - 0.5).ceil()).abs() < 0.0001);
        assert!((covered.top() * ppp - ((y - 0.5).floor() + 1.0)).abs() < 0.0001);
    }
}
