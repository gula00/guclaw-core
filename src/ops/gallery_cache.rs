use base64::{engine::general_purpose::STANDARD, Engine as _};
use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use crate::{DbHandle, SyncGalleryMessageInput};

fn extract_gallery_parts(message_json: &str) -> Vec<(i32, String, Option<String>, Option<String>)> {
    let Ok(root) = serde_json::from_str::<Value>(message_json) else {
        return Vec::new();
    };
    let Some(role) = root.get("role").and_then(Value::as_str) else {
        return Vec::new();
    };
    if role != "assistant" {
        return Vec::new();
    }

    let Some(parts) = root.get("parts").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut out: Vec<(i32, String, Option<String>, Option<String>)> = Vec::new();
    let mut image_index: i32 = 0;
    for part in parts {
        let Some(obj) = part.as_object() else {
            continue;
        };
        let is_file = obj.get("type").and_then(Value::as_str) == Some("file");
        if !is_file {
            continue;
        }

        let Some(media_type) = obj.get("mediaType").and_then(Value::as_str) else {
            continue;
        };
        if !media_type.contains("image") {
            continue;
        }

        let filename = obj
            .get("filename")
            .and_then(Value::as_str)
            .map(str::to_string);
        let url = obj.get("url").and_then(Value::as_str).map(str::to_string);
        out.push((image_index, media_type.to_string(), filename, url));
        image_index += 1;
    }

    out
}

fn mime_to_extension(mime: &str) -> &'static str {
    match mime.to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "bin",
    }
}

fn parse_data_url(url: &str) -> Option<(String, Vec<u8>)> {
    if !url.starts_with("data:") {
        return None;
    }
    let mut parts = url.splitn(2, ',');
    let header = parts.next()?;
    let payload = parts.next()?;
    let mime = header
        .strip_prefix("data:")?
        .strip_suffix(";base64")?
        .to_string();
    let bytes = STANDARD.decode(payload).ok()?;
    Some((mime, bytes))
}

fn compute_image_metadata_and_persist(
    gallery_id: &str,
    media_type: &str,
    url: Option<&str>,
    cache_dir: Option<&str>,
) -> (Option<i32>, Option<i32>, Option<f64>, Option<String>) {
    let Some(url) = url else {
        return (None, None, None, None);
    };
    let Some((mime_from_url, bytes)) = parse_data_url(url) else {
        return (None, None, None, None);
    };

    let (width, height) = imagesize::blob_size(&bytes)
        .map(|dim| (Some(dim.width as i32), Some(dim.height as i32)))
        .unwrap_or((None, None));
    let aspect_ratio = match (width, height) {
        (Some(w), Some(h)) if w > 0 => Some(h as f64 / w as f64),
        _ => None,
    };

    let Some(cache_dir) = cache_dir else {
        return (width, height, aspect_ratio, None);
    };

    let mut file_path = PathBuf::from(cache_dir);
    let ext = mime_to_extension(if media_type.is_empty() {
        &mime_from_url
    } else {
        media_type
    });
    file_path.push(format!("{}.{}", gallery_id, ext));
    if let Some(parent) = file_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if !file_path.exists() {
        let _ = fs::write(&file_path, &bytes);
    }

    (
        width,
        height,
        aspect_ratio,
        Some(file_path.to_string_lossy().to_string()),
    )
}

fn rebuild_gallery_cache(
    conn: &mut rusqlite::Connection,
    cache_dir: Option<&str>,
) -> rusqlite::Result<i32> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM gallery_images", [])?;

    let mut query = tx.prepare(
        "SELECT m.id, m.thread_id, m.message, m.created_at, t.title \
         FROM chat_messages m INNER JOIN chat_threads t ON m.thread_id = t.id \
         ORDER BY m.created_at ASC",
    )?;
    let rows = query
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(query);

    let mut insert = tx.prepare(
        "INSERT OR REPLACE INTO gallery_images \
         (id, message_id, thread_id, thread_title, part_index, media_type, filename, width, height, aspect_ratio, file_path, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;

    let mut inserted = 0;
    for (message_id, thread_id, message_json, created_at, thread_title) in rows {
        for (part_index, media_type, filename, url) in extract_gallery_parts(&message_json) {
            let gallery_id = format!("{}-{}", message_id, part_index);
            let (width, height, aspect_ratio, file_path) = compute_image_metadata_and_persist(
                &gallery_id,
                &media_type,
                url.as_deref(),
                cache_dir,
            );
            insert.execute(params![
                gallery_id,
                message_id,
                thread_id,
                thread_title,
                part_index,
                media_type,
                filename,
                width,
                height,
                aspect_ratio,
                file_path,
                created_at
            ])?;
            inserted += 1;
        }
    }

    drop(insert);
    tx.commit()?;
    Ok(inserted)
}

fn sync_gallery_for_single_message(
    conn: &rusqlite::Connection,
    message_id: &str,
    thread_id: &str,
    thread_title: Option<&str>,
    message_json: &str,
    created_at: &str,
    cache_dir: Option<&str>,
) -> rusqlite::Result<i32> {
    conn.execute(
        "DELETE FROM gallery_images WHERE message_id = ?1",
        [message_id],
    )?;

    let resolved_thread_title = if let Some(title) = thread_title {
        title.to_string()
    } else {
        conn.query_row(
            "SELECT title FROM chat_threads WHERE id = ?1",
            [thread_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    };

    let parts = extract_gallery_parts(message_json);
    if parts.is_empty() {
        return Ok(0);
    }

    let mut insert = conn.prepare(
        "INSERT OR REPLACE INTO gallery_images \
         (id, message_id, thread_id, thread_title, part_index, media_type, filename, width, height, aspect_ratio, file_path, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;

    let mut inserted = 0;
    for (part_index, media_type, filename, url) in parts {
        let gallery_id = format!("{}-{}", message_id, part_index);
        let (width, height, aspect_ratio, file_path) =
            compute_image_metadata_and_persist(&gallery_id, &media_type, url.as_deref(), cache_dir);
        insert.execute(params![
            gallery_id,
            message_id,
            thread_id,
            resolved_thread_title,
            part_index,
            media_type,
            filename,
            width,
            height,
            aspect_ratio,
            file_path,
            created_at
        ])?;
        inserted += 1;
    }

    Ok(inserted)
}

fn backfill_gallery_metadata(
    conn: &rusqlite::Connection,
    cache_dir: Option<&str>,
) -> rusqlite::Result<i32> {
    let mut query = conn.prepare(
        "SELECT g.id, g.message_id, g.part_index, g.media_type, m.message \
         FROM gallery_images g \
         INNER JOIN chat_messages m ON m.id = g.message_id \
         WHERE g.file_path IS NULL OR g.width IS NULL OR g.height IS NULL OR g.aspect_ratio IS NULL",
    )?;
    let rows = query
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut update = conn.prepare(
        "UPDATE gallery_images SET width = ?2, height = ?3, aspect_ratio = ?4, file_path = ?5 WHERE id = ?1",
    )?;

    let mut updated = 0;
    for (gallery_id, _message_id, part_index, media_type, message_json) in rows {
        let part = extract_gallery_parts(&message_json)
            .into_iter()
            .find(|(idx, _, _, _)| *idx == part_index);
        let Some((_, _, _, url)) = part else {
            continue;
        };

        let (width, height, aspect_ratio, file_path) =
            compute_image_metadata_and_persist(&gallery_id, &media_type, url.as_deref(), cache_dir);
        update.execute(params![gallery_id, width, height, aspect_ratio, file_path])?;
        updated += 1;
    }

    Ok(updated)
}

#[napi]
impl DbHandle {
    #[napi]
    pub fn rebuild_gallery_images_cache(&self, cache_dir: Option<String>) -> Result<i32> {
        self.with_connection(|conn| rebuild_gallery_cache(conn, cache_dir.as_deref()))
    }

    #[napi]
    pub fn ensure_gallery_cache_ready(&self, cache_dir: Option<String>) -> Result<bool> {
        self.with_connection(|conn| {
            let has_gallery_table: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='gallery_images')",
                [],
                |row| row.get(0),
            )?;
            if !has_gallery_table {
                return Ok(false);
            }

            let count: i64 = conn.query_row("SELECT COUNT(1) FROM gallery_images", [], |row| {
                row.get(0)
            })?;
            if count > 0 {
                let backfilled = backfill_gallery_metadata(conn, cache_dir.as_deref())?;
                return Ok(backfilled > 0);
            }

            let _ = rebuild_gallery_cache(conn, cache_dir.as_deref())?;
            Ok(true)
        })
    }

    #[napi]
    pub fn sync_gallery_images_for_message(&self, input: SyncGalleryMessageInput) -> Result<i32> {
        self.with_connection(|conn| {
            sync_gallery_for_single_message(
                conn,
                &input.id,
                &input.thread_id,
                input.thread_title.as_deref(),
                &input.message_json,
                &input.created_at,
                input.cache_dir.as_deref(),
            )
        })
    }

    #[napi]
    pub fn delete_gallery_images_for_message(&self, message_id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "DELETE FROM gallery_images WHERE message_id = ?1",
                [message_id],
            )?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn get_gallery_images_cached(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<String> {
        self.with_connection(|conn| {
            let offset = offset.unwrap_or(0).max(0);

            let rows: Vec<Value> = if let Some(limit) = limit {
                let mut stmt = conn.prepare(
                    "SELECT id, message_id, thread_id, thread_title, part_index, media_type, filename, width, height, aspect_ratio, file_path, created_at \
                     FROM gallery_images ORDER BY datetime(created_at) DESC LIMIT ?1 OFFSET ?2",
                )?;
                let mapped = stmt.query_map(params![limit.max(0), offset], |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "messageId": row.get::<_, String>(1)?,
                        "threadId": row.get::<_, String>(2)?,
                        "threadTitle": row.get::<_, String>(3)?,
                        "partIndex": row.get::<_, i32>(4)?,
                        "mediaType": row.get::<_, String>(5)?,
                        "filename": row.get::<_, Option<String>>(6)?,
                        "width": row.get::<_, Option<i32>>(7)?,
                        "height": row.get::<_, Option<i32>>(8)?,
                        "aspectRatio": row.get::<_, Option<f64>>(9)?,
                        "filePath": row.get::<_, Option<String>>(10)?,
                        "createdAt": row.get::<_, String>(11)?,
                    }))
                })?;
                mapped.collect::<rusqlite::Result<Vec<_>>>()?
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, message_id, thread_id, thread_title, part_index, media_type, filename, width, height, aspect_ratio, file_path, created_at \
                     FROM gallery_images ORDER BY datetime(created_at) DESC",
                )?;
                let mapped = stmt.query_map([], |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "messageId": row.get::<_, String>(1)?,
                        "threadId": row.get::<_, String>(2)?,
                        "threadTitle": row.get::<_, String>(3)?,
                        "partIndex": row.get::<_, i32>(4)?,
                        "mediaType": row.get::<_, String>(5)?,
                        "filename": row.get::<_, Option<String>>(6)?,
                        "width": row.get::<_, Option<i32>>(7)?,
                        "height": row.get::<_, Option<i32>>(8)?,
                        "aspectRatio": row.get::<_, Option<f64>>(9)?,
                        "filePath": row.get::<_, Option<String>>(10)?,
                        "createdAt": row.get::<_, String>(11)?,
                    }))
                })?;
                mapped.collect::<rusqlite::Result<Vec<_>>>()?
            };

            let total: i64 = conn.query_row("SELECT COUNT(1) FROM gallery_images", [], |row| {
                row.get(0)
            })?;

            Ok(json!({
                "images": rows,
                "total": total,
            })
            .to_string())
        })
    }

    #[napi]
    pub fn get_gallery_image_by_id_cached(&self, id: String) -> Result<Option<String>> {
        self.with_connection(|conn| {
            let payload = conn
                .query_row(
                    "SELECT id, message_id, thread_id, thread_title, part_index, media_type, filename, width, height, aspect_ratio, file_path, created_at FROM gallery_images WHERE id = ?1",
                    [id],
                    |row| {
                        Ok(json!({
                            "id": row.get::<_, String>(0)?,
                            "messageId": row.get::<_, String>(1)?,
                            "threadId": row.get::<_, String>(2)?,
                            "threadTitle": row.get::<_, String>(3)?,
                            "partIndex": row.get::<_, i32>(4)?,
                            "mediaType": row.get::<_, String>(5)?,
                            "filename": row.get::<_, Option<String>>(6)?,
                            "width": row.get::<_, Option<i32>>(7)?,
                            "height": row.get::<_, Option<i32>>(8)?,
                            "aspectRatio": row.get::<_, Option<f64>>(9)?,
                            "filePath": row.get::<_, Option<String>>(10)?,
                            "createdAt": row.get::<_, String>(11)?,
                        })
                        .to_string())
                    },
                )
                .optional()?;
            Ok(payload)
        })
    }
}
