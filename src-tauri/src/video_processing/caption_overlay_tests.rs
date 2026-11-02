use super::*;
fn fixture() -> CaptionOverlay {
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(2, 2, image::Rgba([240, 200, 100, 128]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let part = Part {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
        opacity: 0.5,
        blur_radius: 5.0,
    };
    CaptionOverlay {
        width: 100,
        height: 100,
        frames: 10,
        tiles: vec![Tile {
            png: STANDARD.encode(png.into_inner()),
        }],
        spans: vec![Span {
            start_frame: 2,
            end_frame: 5,
            background: Some(part.clone()),
            words: vec![Word { tile: 0, part }],
        }],
        background_color: "#11223380".into(),
        corner_radius: 12.0,
    }
}
#[test]
fn decodes_tiles_and_preserves_transparency_and_half_open_ranges() {
    let prepared = Prepared::new(fixture(), 100, 100, 10).unwrap();
    assert_eq!((prepared.tiles[0].width, prepared.tiles[0].height), (2, 2));
    assert_eq!(&prepared.tiles[0].pixels[..4], &[240, 200, 100, 128]);
    assert_eq!(
        prepared.background_color,
        [17.0 / 255.0, 34.0 / 255.0, 51.0 / 255.0, 128.0 / 255.0]
    );
    assert_eq!(prepared.corner_radius, 12.0);
    assert!(prepared.at(1).is_none());
    assert!(prepared.at(2).is_some());
    assert!(prepared.at(4).is_some());
    assert!(prepared.at(5).is_none());
}
#[test]
fn accepts_frontend_flattened_word_contract() {
    let json = serde_json::to_value(fixture()).unwrap();
    assert_eq!(json["spans"][0]["words"][0]["blurRadius"], 5.0);
    assert!(json["spans"][0]["words"][0].get("part").is_none());
    Prepared::new(serde_json::from_value(json).unwrap(), 100, 100, 10).unwrap();
}
#[test]
fn rejects_bad_clocks_geometry_indices_and_images() {
    for mutation in 0..8 {
        let mut input = fixture();
        match mutation {
            0 => input.frames = 9,
            1 => input.spans.push(input.spans[0].clone()),
            2 => input.spans[0].words[0].tile = 1,
            3 => input.spans[0].words[0].part.blur_radius = 11.0,
            4 => input.spans[0].words[0].part.x = f64::NAN,
            5 => input.background_color = "#世界世界".into(),
            6 => input.tiles[0].png = "not png".into(),
            _ => input.spans[0].end_frame = 11,
        }
        assert!(
            Prepared::new(input, 100, 100, 10).is_err(),
            "mutation {mutation}"
        );
    }
}
#[test]
#[ignore = "requires a browser-generated caption payload"]
fn accepts_browser_generated_payload() {
    let path = std::env::var("TARANTINO_CAPTION_PAYLOAD").expect("set TARANTINO_CAPTION_PAYLOAD");
    let input: CaptionOverlay = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let (width, height, frames) = (input.width, input.height, input.frames);
    let prepared = Prepared::new(input, width, height, frames).unwrap();
    assert!(!prepared.tiles.is_empty());
    assert!(!prepared.spans.is_empty());
    for span in &prepared.spans {
        assert!(prepared.at(span.start_frame).is_some());
        for word in &span.words {
            assert!(!prepared.tiles[word.tile].pixels.is_empty());
        }
    }
    println!(
        "Validated {frames} frames, {} text tiles and {} motion spans",
        prepared.tiles.len(),
        prepared.spans.len()
    );
}
