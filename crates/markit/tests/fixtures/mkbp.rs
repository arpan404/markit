use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

// Independent consumer used as a semantic oracle, not the encoder's helpers.
pub fn decode(bytes: &[u8]) -> Value {
    assert_eq!(&bytes[..4], b"MKBP");
    let w = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    assert_eq!(w(4), 1);
    let spans_at = 32 + w(12);
    let blocks_at = spans_at + w(16) * 40;
    let table_at = blocks_at + w(20) * 12;
    let blobs_at = table_at + w(24) * 8;
    assert_eq!(blobs_at + w(28), bytes.len());
    let blob = |id: usize| {
        let at = table_at + id * 8;
        &bytes[blobs_at + w(at)..blobs_at + w(at) + w(at + 4)]
    };
    let string = |id| String::from_utf8(blob(id).to_vec()).unwrap();
    let mut spans = vec![];
    for i in 0..w(16) {
        let at = spans_at + i * 40;
        let a = |i: usize| w(at + 12 + i * 4);
        let mut style = match w(at + 8) {
            1 => json!({"kind":"heading","level":a(0)}),
            2 => json!({"kind":"paragraph"}),
            3 => json!({"kind":"strong"}),
            4 => json!({"kind":"emphasis"}),
            5 => json!({"kind":"strikethrough"}),
            6 => json!({"kind":"code"}),
            7 => json!({"kind":"code_block","language":string(a(0))}),
            8 => json!({"kind":"link","url":string(a(0))}),
            9 => json!({"kind":"image","url":string(a(0))}),
            10 => json!({"kind":"quote","depth":a(0)}),
            11 => json!({"kind":"list_item","depth":a(0)}),
            12 => json!({"kind":"table"}),
            13 => json!({"kind":"table_header"}),
            14 => json!({"kind":"rule"}),
            15 => json!({"kind":"reference","reference_kind":string(a(0)),"target":string(a(1))}),
            16 => {
                json!({"kind":"code_metadata","file":if a(0)==u32::MAX as usize {Value::Null} else {json!(string(a(0)))},"diff":a(1)!=0})
            }
            17 => {
                let palette = [
                    ("#24292f", "#c9d1d9"),
                    ("#6e7781", "#8b949e"),
                    ("#0a3069", "#a5d6ff"),
                    ("#0550ae", "#79c0ff"),
                    ("#cf222e", "#ff7b72"),
                    ("#8250df", "#d2a8ff"),
                    ("#953800", "#ffa657"),
                    ("#116329", "#7ee787"),
                ];
                let (l, d) = palette[a(0)];
                json!({"kind":"syntax","light":l,"dark":d})
            }
            18 => json!({"kind":"diff","change":string(a(0))}),
            19 => {
                assert!(blob(a(3)).starts_with(b"\x89PNG\r\n\x1a\n"));
                json!({"kind":"math","tex":string(a(0)),"raw":string(a(1)),"display":a(2)!=0,"png":STANDARD.encode(blob(a(3))),"width":a(4),"height":a(5)})
            }
            20 => {
                json!({"kind":"table_cell","row":a(0),"column":a(1),"header":a(2)!=0,"alignment":string(a(3))})
            }
            21 => {
                json!({"kind":"syntax","light":format!("#{:06x}",a(0)),"dark":format!("#{:06x}",a(1))})
            }
            22 => json!({"kind":"syntax","light":string(a(0)),"dark":string(a(1))}),
            23 => json!({"kind":"alert","alert":string(a(0))}),
            24 => json!({"kind":"footnote_reference","label":string(a(0)),"number":a(1)}),
            25 => json!({"kind":"footnote_definition","label":string(a(0)),"number":a(1)}),
            26 => json!({"kind":"task","index":a(0),"offset":a(1),"checked":a(2)!=0}),
            27 => json!({"kind":"diagram","language":string(a(0)),"source":string(a(1))}),
            28 => json!({"kind":"video","url":string(a(0))}),
            29 => json!({"kind":"html","tag":string(a(0))}),
            30 => json!({"kind":"dark_image","url":string(a(0))}),
            tag => panic!("unknown tag {tag}"),
        };
        style["start"] = json!(w(at));
        style["end"] = json!(w(at + 4));
        spans.push(style);
    }
    let blocks: Vec<_> = (0..w(20))
        .map(|i| {
            let at = blocks_at + i * 12;
            json!({"start":w(at),"end":w(at+4),"kind":string(w(at+8))})
        })
        .collect();
    json!({"version":w(8),"text":std::str::from_utf8(&bytes[32..spans_at]).unwrap(),"spans":spans,"blocks":blocks})
}
