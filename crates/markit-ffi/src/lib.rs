use std::{panic::catch_unwind, ptr, slice, str};

#[cfg(target_os = "android")]
mod android;

/// Owned bytes, UTF-8 for JSON/HTML and binary for the binary API. Release exactly once with markit_buffer_free.
/// status: 0 success, 1 invalid UTF-8, 2 invalid argument, 3 internal panic.
#[repr(C)]
pub struct MarkitBuffer {
    pub data: *mut u8,
    pub len: usize,
    pub status: u32,
}

impl MarkitBuffer {
    fn error(status: u32) -> Self {
        Self {
            data: ptr::null_mut(),
            len: 0,
            status,
        }
    }
    fn owned(text: String) -> Self {
        Self::bytes(text.into_bytes())
    }
    fn bytes(bytes: Vec<u8>) -> Self {
        let bytes = bytes.into_boxed_slice();
        let len = bytes.len();
        Self {
            data: Box::into_raw(bytes) as *mut u8,
            len,
            status: 0,
        }
    }
}

/// Borrowed code request. No JSON request allocation. mode 0 lexical, 1 grammar.
#[repr(C)]
pub struct MarkitCodeRequest {
    pub source: *const u8,
    pub source_len: usize,
    pub language: *const u8,
    pub language_len: usize,
    pub mode: u32,
}

unsafe fn input<'a>(data: *const u8, len: usize) -> Result<&'a str, u32> {
    if (data.is_null() && len != 0) || len > isize::MAX as usize {
        return Err(2);
    }
    let bytes = if len == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(data, len) }
    };
    str::from_utf8(bytes).map_err(|_| 1)
}
fn binary_result(result: Result<Vec<u8>, markit::BinaryError>) -> MarkitBuffer {
    match result {
        Ok(bytes) => MarkitBuffer::bytes(bytes),
        Err(_) => MarkitBuffer::error(2),
    }
}

/// # Safety
/// Source must be readable for len bytes, as for markit_render. Free with markit_buffer_free.
#[no_mangle]
pub unsafe extern "C" fn markit_render_binary(source: *const u8, len: usize) -> MarkitBuffer {
    catch_unwind(|| match unsafe { input(source, len) } {
        Ok(source) => binary_result(markit::render_document_binary(source)),
        Err(status) => MarkitBuffer::error(status),
    })
    .unwrap_or_else(|_| MarkitBuffer::error(3))
}

/// # Safety
/// request points to a readable struct; its source and language are borrowed readable
/// UTF-8 slices for this call only. No pointers are retained. Free output exactly once.
#[no_mangle]
pub unsafe extern "C" fn markit_code_binary(request: *const MarkitCodeRequest) -> MarkitBuffer {
    if request.is_null() {
        return MarkitBuffer::error(2);
    }
    catch_unwind(|| {
        let r = unsafe { &*request };
        let source = match unsafe { input(r.source, r.source_len) } {
            Ok(s) => s,
            Err(n) => return MarkitBuffer::error(n),
        };
        let language = match unsafe { input(r.language, r.language_len) } {
            Ok(s) => s,
            Err(n) => return MarkitBuffer::error(n),
        };
        let doc = match r.mode {
            0 => markit::code_document(source, language),
            1 => match markit::code_document_grammar(source, language) {
                Some(d) => d,
                None => return MarkitBuffer::error(2),
            },
            _ => return MarkitBuffer::error(2),
        };
        binary_result(markit::encode_document_binary(&doc))
    })
    .unwrap_or_else(|_| MarkitBuffer::error(3))
}

/// format 0 = native JSON, 1 = safe HTML, 2 = code document request JSON.
///
/// # Safety
/// For nonzero len, source must point to len readable bytes for the duration of
/// this call. len must not exceed isize::MAX. No pointer is retained.
#[no_mangle]
pub unsafe extern "C" fn markit_render(source: *const u8, len: usize, format: u32) -> MarkitBuffer {
    if (source.is_null() && len != 0) || len > isize::MAX as usize || format > 3 {
        return MarkitBuffer::error(2);
    }
    catch_unwind(|| {
        let bytes = if len == 0 {
            &[][..]
        } else {
            // SAFETY: the caller guarantees source is readable for len bytes.
            unsafe { slice::from_raw_parts(source, len) }
        };
        let Ok(text) = str::from_utf8(bytes) else {
            return MarkitBuffer::error(1);
        };
        MarkitBuffer::owned(if format >= 2 {
            let Ok(request) = serde_json::from_str::<serde_json::Value>(text) else {
                return MarkitBuffer::error(2);
            };
            let (Some(source), Some(language)) =
                (request["source"].as_str(), request["language"].as_str())
            else {
                return MarkitBuffer::error(2);
            };
            let document = if format == 3 {
                let Some(document) = markit::code_document_grammar(source, language) else {
                    return MarkitBuffer::error(2);
                };
                document
            } else {
                markit::code_document(source, language)
            };
            serde_json::to_string(&document).expect("serializable document")
        } else if format == 0 {
            markit::render_document_json(text)
        } else {
            markit::render_html(text)
        })
    })
    .unwrap_or_else(|_| MarkitBuffer::error(3))
}

#[no_mangle]
pub extern "C" fn markit_stream_new() -> *mut markit::StreamRenderer {
    Box::into_raw(Box::new(markit::StreamRenderer::new_native()))
}

/// # Safety
/// stream must be a live handle from markit_stream_new, used by one caller at a
/// time. source has the same readable-byte requirements as markit_render.
#[no_mangle]
pub unsafe extern "C" fn markit_stream_append(
    stream: *mut markit::StreamRenderer,
    source: *const u8,
    len: usize,
    finalized: u8,
) -> MarkitBuffer {
    if stream.is_null() || (source.is_null() && len != 0) || len > isize::MAX as usize {
        return MarkitBuffer::error(2);
    }
    catch_unwind(|| {
        // SAFETY: caller owns a live, exclusively accessed stream and readable input.
        let stream = unsafe { &mut *stream };
        let input = if len == 0 {
            &[][..]
        } else {
            unsafe { slice::from_raw_parts(source, len) }
        };
        let Ok(delta) = str::from_utf8(input) else {
            return MarkitBuffer::error(1);
        };
        if stream.source().len() + len > 8 * 1024 * 1024 {
            return MarkitBuffer::error(2);
        }
        MarkitBuffer::owned(
            serde_json::to_string(&stream.append(delta, finalized != 0))
                .expect("serializable patch"),
        )
    })
    .unwrap_or_else(|_| MarkitBuffer::error(3))
}

/// # Safety
/// Same requirements as [`markit_stream_append`]. The buffer is an MKNP v1 patch.
#[no_mangle]
pub unsafe extern "C" fn markit_stream_append_binary(
    stream: *mut markit::StreamRenderer,
    source: *const u8,
    len: usize,
    finalized: u8,
) -> MarkitBuffer {
    if stream.is_null() || (source.is_null() && len != 0) || len > isize::MAX as usize {
        return MarkitBuffer::error(2);
    }
    catch_unwind(|| {
        let stream = unsafe { &mut *stream };
        let input = if len == 0 {
            &[][..]
        } else {
            unsafe { slice::from_raw_parts(source, len) }
        };
        let Ok(delta) = str::from_utf8(input) else {
            return MarkitBuffer::error(1);
        };
        if stream.source().len() + len > 8 * 1024 * 1024 {
            return MarkitBuffer::error(2);
        }
        binary_result(markit::encode_native_patch(
            &stream.append(delta, finalized != 0),
        ))
    })
    .unwrap_or_else(|_| MarkitBuffer::error(3))
}

/// # Safety
/// Free a handle returned by markit_stream_new exactly once, with no call in flight.
#[no_mangle]
pub unsafe extern "C" fn markit_stream_free(stream: *mut markit::StreamRenderer) {
    if !stream.is_null() {
        drop(unsafe { Box::from_raw(stream) });
    }
}

/// # Safety
/// Pass an unmodified buffer returned by any render API, exactly once. Null error
/// buffers are accepted. Copies of a buffer do not transfer additional ownership.
#[no_mangle]
pub unsafe extern "C" fn markit_buffer_free(buffer: MarkitBuffer) {
    if !buffer.data.is_null() {
        // SAFETY: same pointer and length returned from Box::into_raw in owned().
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(buffer.data, buffer.len)) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_exports_preserve_plan_and_validate_borrowed_requests() {
        let source = "const 🦀 = 1;";
        let language = "ts";
        let request = MarkitCodeRequest {
            source: source.as_ptr(),
            source_len: source.len(),
            language: language.as_ptr(),
            language_len: language.len(),
            mode: 0,
        };
        let code = unsafe { markit_code_binary(&request) };
        assert_eq!(code.status, 0);
        assert_eq!(
            unsafe { slice::from_raw_parts(code.data, code.len) },
            markit::encode_document_binary(&markit::code_document(source, language)).unwrap()
        );
        unsafe { markit_buffer_free(code) };
        let doc = unsafe { markit_render_binary(source.as_ptr(), source.len()) };
        assert_eq!(doc.status, 0);
        assert_eq!(
            unsafe { slice::from_raw_parts(doc.data, doc.len) },
            markit::render_document_binary(source).unwrap()
        );
        unsafe { markit_buffer_free(doc) };
        for result in [
            unsafe { markit_code_binary(ptr::null()) },
            unsafe { markit_render_binary(ptr::null(), 1) },
            unsafe { markit_render_binary(b"\xff".as_ptr(), 1) },
            unsafe {
                markit_code_binary(&MarkitCodeRequest {
                    mode: 99,
                    ..request
                })
            },
        ] {
            assert_ne!(result.status, 0);
            unsafe { markit_buffer_free(result) };
        }
        let empty = unsafe { markit_render_binary(ptr::null(), 0) };
        assert_eq!(empty.status, 0);
        unsafe { markit_buffer_free(empty) };
    }

    #[test]
    fn roundtrip_and_free_with_embedded_nul() {
        let source = "# Hello 🦀\0";
        for format in [0, 1] {
            let result = unsafe { markit_render(source.as_ptr(), source.len(), format) };
            assert_eq!(result.status, 0);
            let text = unsafe { slice::from_raw_parts(result.data, result.len) };
            assert!(str::from_utf8(text).unwrap().contains("Hello"));
            unsafe { markit_buffer_free(result) };
        }
    }

    #[test]
    fn rejects_bad_inputs_and_accepts_empty() {
        for (data, len, format, expected) in [
            (ptr::null(), 1, 0, 2),
            (ptr::null(), 0, 99, 2),
            (b"\xff".as_ptr(), 1, 0, 1),
            (ptr::null(), 0, 0, 0),
        ] {
            let buffer = unsafe { markit_render(data, len, format) };
            assert_eq!(buffer.status, expected);
            unsafe { markit_buffer_free(buffer) };
        }
    }

    #[test]
    fn code_requests_and_stream_handles_roundtrip_real_json() {
        let request = br#"{"source":"let x = 1;","language":"rust"}"#;
        let buffer = unsafe { markit_render(request.as_ptr(), request.len(), 2) };
        assert_eq!(buffer.status, 0);
        let document: serde_json::Value =
            serde_json::from_slice(unsafe { slice::from_raw_parts(buffer.data, buffer.len) })
                .unwrap();
        assert_eq!(document["text"], "let x = 1;");
        assert!(document["spans"]
            .as_array()
            .unwrap()
            .iter()
            .any(|span| span["kind"] == "syntax"));
        unsafe {
            markit_buffer_free(buffer);
        }
        let handle = markit_stream_new();
        for delta in ["First\n\nOpen", " tail\n"] {
            let patch = unsafe { markit_stream_append(handle, delta.as_ptr(), delta.len(), 0) };
            assert_eq!(patch.status, 0);
            let value: serde_json::Value =
                serde_json::from_slice(unsafe { slice::from_raw_parts(patch.data, patch.len) })
                    .unwrap();
            assert!(value["blocks"].is_array());
            unsafe {
                markit_buffer_free(patch);
            }
        }
        let binary = unsafe { markit_stream_append_binary(handle, b"\n".as_ptr(), 1, 1) };
        assert_eq!(binary.status, 0);
        let bytes = unsafe { slice::from_raw_parts(binary.data, binary.len) };
        assert_eq!(&bytes[..4], b"MKNP");
        unsafe {
            markit_buffer_free(binary);
            markit_stream_free(handle);
        }
        let error = unsafe { markit_stream_append(ptr::null_mut(), ptr::null(), 0, 1) };
        assert_eq!(error.status, 2);
    }
}
