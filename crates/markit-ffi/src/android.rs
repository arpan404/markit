use jni::{
    objects::{JClass, JString},
    sys::{jboolean, jint, jlong, jstring},
    JNIEnv,
};
use std::{
    collections::HashMap,
    ptr,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

type Streams = HashMap<i64, Arc<Mutex<markit::StreamRenderer>>>;
static STREAMS: OnceLock<Mutex<Streams>> = OnceLock::new();
static NEXT: AtomicI64 = AtomicI64::new(1);
fn streams() -> &'static Mutex<Streams> {
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn invoke(
    env: &mut JNIEnv,
    operation: impl FnOnce(&mut JNIEnv) -> Result<String, String>,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(env)));
    let text = match result {
        Ok(Ok(text)) => text,
        other => {
            let error = match other {
                Ok(Err(error)) => error,
                _ => "Markit rendering failed".into(),
            };
            if !env.exception_check().unwrap_or(true) {
                let _ = env.throw_new("java/lang/IllegalStateException", error);
            }
            return ptr::null_mut();
        }
    };
    env.new_string(text)
        .map(|text| text.into_raw())
        .unwrap_or(ptr::null_mut())
}
#[no_mangle]
pub extern "system" fn Java_dev_markit_Markit_nativeRender(
    mut env: JNIEnv,
    _: JClass,
    source: JString,
    format: jint,
) -> jstring {
    invoke(&mut env, |env| {
        let source: String = env.get_string(&source).map_err(|e| e.to_string())?.into();
        if source.len() > 16 * 1024 * 1024 {
            return Err("Document exceeds 16 MiB".into());
        }
        let document = if format == 2 || format == 3 {
            let request: serde_json::Value =
                serde_json::from_str(&source).map_err(|e| e.to_string())?;
            let (Some(source), Some(language)) =
                (request["source"].as_str(), request["language"].as_str())
            else {
                return Err("Invalid code request".into());
            };
            if format == 3 {
                markit::code_document_grammar(source, language)
                    .ok_or("Build with the grammars feature")?
            } else {
                markit::code_document(source, language)
            }
        } else if format == 0 {
            markit::parse(&source)
        } else {
            return Err("Unknown format".into());
        };
        serde_json::to_string(&document).map_err(|e| e.to_string())
    })
}
#[no_mangle]
pub extern "system" fn Java_dev_markit_Markit_nativeCreateStream(
    mut env: JNIEnv,
    _: JClass,
) -> jlong {
    let Ok(mut registry) = streams().lock() else {
        let _ = env.throw_new(
            "java/lang/IllegalStateException",
            "Stream registry unavailable",
        );
        return 0;
    };
    if registry.len() >= 64 {
        let _ = env.throw_new(
            "java/lang/IllegalStateException",
            "Close unused Markdown streams",
        );
        return 0;
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    registry.insert(
        id,
        Arc::new(Mutex::new(markit::StreamRenderer::new_native())),
    );
    id
}
#[no_mangle]
pub extern "system" fn Java_dev_markit_Markit_nativeAppend(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    delta: JString,
    finalized: jboolean,
) -> jstring {
    invoke(&mut env, |env| {
        let delta: String = env.get_string(&delta).map_err(|e| e.to_string())?.into();
        let stream = streams()
            .lock()
            .map_err(|e| e.to_string())?
            .get(&id)
            .cloned()
            .ok_or("Closed stream")?;
        let mut stream = stream.lock().map_err(|e| e.to_string())?;
        if stream.source().len() + delta.len() > 8 * 1024 * 1024 {
            return Err("Document exceeds 8 MiB".into());
        }
        serde_json::to_string(&stream.append(&delta, finalized != 0)).map_err(|e| e.to_string())
    })
}
#[no_mangle]
pub extern "system" fn Java_dev_markit_Markit_nativeFreeStream(_: JNIEnv, _: JClass, id: jlong) {
    if let Ok(mut registry) = streams().lock() {
        registry.remove(&id);
    }
}
