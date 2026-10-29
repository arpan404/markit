#[path = "fixtures/stream_sequences.rs"]
mod fixture;

#[test]
fn append_patch_bytes_match_baseline_for_fences_references_tables_and_math() {
    // Golden FNV-1a of every serialized patch (HTML fence chrome updated on
    // ace/web-runtime: diff rows, header stats). Includes replace
    // offsets, omitted fields, span order, HTML bytes and native PNG bytes.
    // Exercise all three output modes, including partial lines and closers.
    let mut hash = 0xcbf29ce484222325u64;
    for parts in fixture::sequences() {
        for mode in 0..3 {
            let mut stream = match mode {
                1 => markit::StreamRenderer::new_html(),
                2 => markit::StreamRenderer::new_native(),
                _ => markit::StreamRenderer::new(),
            };
            for (i, part) in parts.iter().enumerate() {
                let patch = stream.append(part, i + 1 == parts.len());
                let bytes = serde_json::to_vec(&patch).unwrap();
                for byte in bytes.into_iter().chain(*b"\n") {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                }
            }
        }
    }
    assert_eq!(hash, 0xc3eb56423c383b41);
}
