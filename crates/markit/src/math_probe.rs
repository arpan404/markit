use super::*;
use std::{hint::black_box, time::Instant};
const CASES: [(&str, &str, bool); 4] = [
    ("x2", "x^2+1", false),
    ("sum_fraction", r"\sum_{i=1}^{n}\frac{i^2}{2}+1", false),
    (
        "display_math",
        r"\int_0^\infty e^{-x^2}\,dx=\frac{\sqrt{\pi}}{2}",
        true,
    ),
    ("invalid_tex", r"\frac{", false),
];
fn clear() {
    CACHE.with(|c| *c.borrow_mut() = crate::cache::Cache::new(4 * 1024 * 1024));
}
#[test]
#[ignore]
fn math_probe() {
    let dir = std::env::var("MATH_PROBE_DIR").unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    for (name, tex, display) in CASES {
        clear();
        for (mode, output) in [
            ("svg", MathOutput::Svg),
            ("png", MathOutput::Pixels),
            ("both", MathOutput::Both),
        ] {
            let result = render_visual(tex, display, output);
            let prefix = format!("{dir}/{name}-{mode}");
            if let Some(v) = result {
                std::fs::write(format!("{prefix}.svg"), &v.svg).unwrap();
                std::fs::write(format!("{prefix}.png"), STANDARD.decode(&v.png).unwrap()).unwrap();
                std::fs::write(
                    format!("{prefix}.dims"),
                    format!("{},{}", v.width, v.height),
                )
                .unwrap();
            } else {
                std::fs::write(format!("{prefix}.none"), b"none").unwrap();
            }
        }
    }
    for kind in [
        "mixed_cold",
        "mixed_warm",
        "invalid_repeat",
        "valid_warm",
        "pair_cold",
        "png_cold",
        "svg_cold",
    ] {
        let count = if kind == "invalid_repeat" || kind.ends_with("warm") {
            10000
        } else {
            100
        };
        clear();
        if kind.ends_with("warm") {
            for (_, tex, display) in CASES {
                for output in [MathOutput::Svg, MathOutput::Pixels, MathOutput::Both] {
                    black_box(render_visual(tex, display, output));
                }
            }
        }
        let start = Instant::now();
        for _ in 0..count {
            if kind.ends_with("cold") {
                clear();
            }
            for (_, tex, display) in CASES {
                if kind == "invalid_repeat" && tex != r"\frac{" {
                    continue;
                }
                if matches!(kind, "valid_warm" | "pair_cold" | "png_cold" | "svg_cold")
                    && tex == r"\frac{"
                {
                    continue;
                }
                for output in [MathOutput::Svg, MathOutput::Pixels, MathOutput::Both] {
                    if kind == "pair_cold" && output == MathOutput::Both {
                        continue;
                    }
                    if kind == "png_cold" && output != MathOutput::Pixels {
                        continue;
                    }
                    if kind == "svg_cold" && output != MathOutput::Svg {
                        continue;
                    }
                    black_box(render_visual(black_box(tex), display, output));
                }
            }
        }
        println!("BENCH {kind} {}", start.elapsed().as_nanos() / count);
    }
}
