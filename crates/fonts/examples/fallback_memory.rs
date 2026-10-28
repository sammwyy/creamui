#[cfg(target_os = "linux")]
fn sample(label: &str) {
    let text = std::fs::read_to_string("/proc/self/smaps").unwrap();
    let mut font = false;
    let mut mappings = 0;
    let (mut size, mut rss, mut pss) = (0_u64, 0_u64, 0_u64);
    for line in text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.first().is_some_and(|field| field.contains('-')) {
            let path = fields.get(5..).unwrap_or_default().join(" ");
            let path = path.strip_suffix(" (deleted)").unwrap_or(&path);
            font = std::path::Path::new(path)
                .extension()
                .is_some_and(|extension| {
                    ["ttf", "otf", "ttc"]
                        .iter()
                        .any(|expected| extension.eq_ignore_ascii_case(expected))
                });
            mappings += usize::from(font);
        } else if font {
            let value = fields
                .get(1)
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            match fields.first().copied() {
                Some("Size:") => size += value,
                Some("Rss:") => rss += value,
                Some("Pss:") => pss += value,
                _ => {}
            }
        }
    }
    println!("{label}\t{mappings}\t{size}\t{rss}\t{pss}");
}

#[cfg(target_os = "linux")]
fn main() {
    use creamui_fonts::{layout, resolve, FontWeight, LayoutSettings, DEFAULT_FAMILY};
    println!("stage\tfont_mappings\tfont_size_kib\tfont_rss_kib\tfont_pss_kib");
    let primary = resolve(DEFAULT_FAMILY, FontWeight::Regular);
    sample("primary");
    let started = std::time::Instant::now();
    drop(layout(
        &primary,
        "\u{10ffff}",
        16.0,
        &LayoutSettings::default(),
    ));
    eprintln!("first_missing_us={}", started.elapsed().as_micros());
    sample("missing");
    let started = std::time::Instant::now();
    for _ in 0..100 {
        drop(layout(
            &primary,
            "\u{10ffff}",
            16.0,
            &LayoutSettings::default(),
        ));
    }
    eprintln!("repeated_missing_us={}", started.elapsed().as_micros());
    sample("repeated_missing");
    let started = std::time::Instant::now();
    for codepoint in 0xfdd0..=0xfdef {
        let text = char::from_u32(codepoint).unwrap().to_string();
        drop(layout(&primary, &text, 16.0, &LayoutSettings::default()));
    }
    eprintln!("distinct_missing_us={}", started.elapsed().as_micros());
    sample("distinct_missing");
    for text in ["English עברית", "中文", "🙂", "A\u{301}"] {
        drop(layout(&primary, text, 16.0, &LayoutSettings::default()));
    }
    sample("mixed_scripts");
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("This probe requires Linux /proc/self/smaps");
}
