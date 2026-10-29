#[path = "fixtures/stream_sequences.rs"]
mod fixture;

#[test]
fn append_patch_bytes_match_baseline_for_fences_references_tables_and_math() {
    // Golden FNV-1a of every serialized patch at 6e578a9, re-recorded when
    // unknown fence languages gained the generic lexer: only the transient
    // fences whose info string is a partial or unknown name (`r`, `pyt`,
    // `rust&amp;`) changed. Includes replace offsets, omitted fields, span
    // order, HTML bytes and native PNG bytes.
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
    assert_eq!(hash, 0x9707859ad7773229);
}
