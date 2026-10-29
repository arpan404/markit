//! Growing a fence reuses the previous scan, including the plain fallback
//! once a fence passes the run limit. Every streamed result must equal a
//! cold scan of the same text on a thread with empty caches.

fn cold(code: &str, language: &str) -> (Vec<u32>, String) {
    let (code, language) = (code.to_owned(), language.to_owned());
    std::thread::spawn(move || {
        (
            markit::highlight_packed(&code, &language),
            markit::highlight_json(&code, &language),
        )
    })
    .join()
    .unwrap()
}

fn sources() -> Vec<(&'static str, String)> {
    let dense: String = (0..1400)
        .map(|i| format!("let a{i} = b + c * d - e / f; // {i}\n"))
        .collect();
    // A block comment opened after the run limit changes how earlier text
    // could be read; the fallback must not stick in that case.
    let late_comment = format!("{}/* open\nstill comment\n*/ let x = 1;\n", dense);
    let normal: String = (0..300)
        .map(|i| format!("fn f{i}(x: u8) -> &'static str {{ \"s{i}\" }} /* c */\n"))
        .collect();
    vec![
        ("rust", dense),
        ("rust", late_comment),
        ("rust", normal),
        (
            "python",
            "x = '''\n".repeat(3) + &"y = 1 + 2\n".repeat(2000),
        ),
    ]
}

#[test]
fn streamed_lexing_matches_cold_scans() {
    for (language, source) in sources() {
        let mut cuts: Vec<usize> = source.match_indices('\n').map(|(at, _)| at + 1).collect();
        cuts.extend((0..source.len()).step_by(97));
        cuts.push(source.len());
        cuts.sort_unstable();
        cuts.dedup();
        let mut checked = 0;
        for (n, &at) in cuts.iter().enumerate() {
            let prefix = &source[..at];
            let streamed = (
                markit::highlight_packed(prefix, language),
                markit::highlight_json(prefix, language),
            );
            // Cold scans are slow; check a spread of prefixes and the tail.
            if n % 37 == 0 || n + 3 >= cuts.len() {
                assert_eq!(streamed, cold(prefix, language), "{language} at {at}");
                checked += 1;
            }
        }
        assert!(checked > 10);
    }
}
