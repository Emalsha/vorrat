//! Pictures and documents attached to an item.

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};

use crate::web::respond::{back, done, not_found, Res};
use crate::web::St;

pub(super) const MAX_UPLOAD: usize = 25 * 1024 * 1024;

/// Stores a picture or PDF for an item. It trusts the bytes, not the browser's
/// label: only real images and PDFs are kept.
pub(super) async fn upload(
    State(s): St,
    Path(id): Path<i64>,
    mut mp: axum::extract::Multipart,
) -> Response {
    let to = format!("/items/{id}");
    let mut as_picture = false;
    let mut file: Option<(String, String, axum::body::Bytes)> = None;
    loop {
        match mp.next_field().await {
            Ok(Some(field)) => match field.name() {
                Some("as_picture") => as_picture = true,
                Some("file") => {
                    let fname = field.file_name().unwrap_or("file").to_string();
                    let ctype = field.content_type().unwrap_or("").to_string();
                    match field.bytes().await {
                        Ok(b) => file = Some((fname, ctype, b)),
                        Err(_) => return back(&to, "Upload failed (max 25 MB)"),
                    }
                }
                _ => {}
            },
            Ok(None) => break,
            Err(_) => return back(&to, "Upload failed (max 25 MB)"),
        }
    }
    let Some((fname, _ctype, bytes)) = file else {
        return back(&to, "Choose a file");
    };
    let Some((mime, ext)) = sniff(&bytes) else {
        return back(&to, "Only photos (JPEG, PNG, WebP) and PDFs");
    };
    if bytes.len() > MAX_UPLOAD {
        return back(&to, "File too large (max 25 MB)");
    }
    let r = (|| -> anyhow::Result<&'static str> {
        if s.store.item(id)?.is_none() {
            anyhow::bail!("no such item");
        }
        let stored = crate::files::new_name("att", ext);
        crate::files::write(&s.cfg.files_dir(), &stored, &bytes)?;
        if as_picture && mime.starts_with("image/") {
            s.store.set_image(id, Some(&stored))?;
            return Ok("Picture updated");
        }
        let clean: String = fname
            .chars()
            .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
            .take(120)
            .collect();
        s.store
            .add_attachment(id, &clean, &stored, mime, bytes.len() as i64)?;
        Ok("Attached")
    })();
    done(&to, r.map(String::from))
}

/// File type from its first bytes.
fn sniff(b: &[u8]) -> Option<(&'static str, &'static str)> {
    if b.starts_with(b"\xFF\xD8\xFF") {
        Some(("image/jpeg", "jpg"))
    } else if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else if b.starts_with(b"%PDF-") {
        Some(("application/pdf", "pdf"))
    } else {
        None
    }
}

/// Serves an attachment with a sandbox policy, so a PDF opened inline can't run
/// scripts in Vorrat's origin.
pub(super) async fn get_attachment(State(s): St, Path(id): Path<i64>) -> Res<Response> {
    let Some(a) = s.store.attachment(id)? else {
        return Ok(not_found());
    };
    if !crate::files::is_safe_name(&a.stored) {
        return Ok(not_found());
    }
    let Ok(bytes) = tokio::fs::read(s.cfg.files_dir().join(&a.stored)).await else {
        return Ok(not_found());
    };
    let ascii: String = a
        .filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || ".-_ ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut res = (
        [
            (
                header::CONTENT_TYPE,
                crate::files::content_type(&a.stored).to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{ascii}\""),
            ),
        ],
        bytes,
    )
        .into_response();
    res.headers_mut().insert("content-security-policy", HeaderValue::from_static("sandbox; default-src 'none'; img-src 'self'; style-src 'unsafe-inline'; plugin-types application/pdf"));
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(res)
}

pub(super) async fn delete_attachment(State(s): St, Path(id): Path<i64>) -> Res<Response> {
    let Some(a) = s.store.attachment(id)? else {
        return Ok(not_found());
    };
    s.store.delete_attachment(id)?;
    if crate::files::is_safe_name(&a.stored) {
        let _ = std::fs::remove_file(s.cfg.files_dir().join(&a.stored));
    }
    Ok(back(&format!("/items/{}", a.item_id), "Removed"))
}
