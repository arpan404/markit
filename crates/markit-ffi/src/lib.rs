use std::{panic::catch_unwind, ptr, slice, str};

#[cfg(target_os = "android")]
mod android;

/// Owned UTF-8 bytes. Release exactly once with markit_buffer_free.
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
        let bytes = text.into_bytes().into_boxed_slice();
        let len = bytes.len();
        Self {
            data: Box::into_raw(bytes) as *mut u8,
            len,
            status: 0,
        }
    }
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
/// Free a handle returned by markit_stream_new exactly once, with no call in flight.
#[no_mangle]
pub unsafe extern "C" fn markit_stream_free(stream: *mut markit::StreamRenderer) {
    if !stream.is_null() {
        drop(unsafe { Box::from_raw(stream) });
    }
}

/// # Safety
/// Pass an unmodified buffer returned by markit_render, exactly once. Null error
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
        unsafe {
            markit_stream_free(handle);
        }
        let error = unsafe { markit_stream_append(ptr::null_mut(), ptr::null(), 0, 1) };
        assert_eq!(error.status, 2);
    }
}
