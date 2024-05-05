use actix_multipart::Multipart;
use actix_web::{get, HttpResponse, put, Responder};
use actix_web::error::{ErrorBadRequest, ErrorInternalServerError};
use actix_web::web::{Json, Path, Query, Redirect};
use chrono::{DateTime, Utc};
use futures_util::{StreamExt, TryStreamExt};
use log::{debug, error, info};
use once_cell::sync::Lazy;
use redis::{FromRedisValue, RedisResult, RedisWrite, ToRedisArgs, Value};
use s3::serde_types::Part;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{API_URL, S3_BUCKET};
use crate::cache::{cache_clear, cache_get, cache_set};

#[derive(Deserialize)]
struct QueryParams {
    cursor: Option<String>,
    limit: Option<usize>,
}

const MAX_LIMIT: Lazy<Option<usize>> = Lazy::new(|| option_env!("S3_LISTOBJECTS_LIMIT").map(|s| s.parse::<usize>().ok().filter(|&num| num > 0).expect("S3_LISTOBJECTS_LIMIT must be a number greater than 0")));
const LISTOBJECTS_DELIMITER: &'static str = "/";

const EXPIRY_TIME: u32 = 60 * 60; // 1 hour

#[derive(Serialize, Deserialize, Clone)]
struct ViewResponse {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<Vec<ViewFileResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    folders: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ViewFileResponse {
    name: String,
    last_modified: DateTime<Utc>,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_url: Option<String>,
}

impl FromRedisValue for ViewResponse {
    fn from_redis_value(v: &Value) -> RedisResult<Self> {
        match *v {
            Value::Data(ref bytes) => {
                Ok(serde_json::from_slice(bytes.as_slice()).expect("Unable to parse JSON"))
            }
            _ => Err(redis::RedisError::from((redis::ErrorKind::TypeError, "Invalid value type")))
        }
    }
}

impl ToRedisArgs for ViewResponse {
    fn write_redis_args<W>(&self, out: &mut W) where W: ?Sized + RedisWrite {
        let vec = serde_json::to_vec(self).expect("Unable to serialize JSON");
        out.write_arg(vec.as_slice());
    }
}

#[get("/view/{path:.*}")]
pub async fn list_s3(path: Path<String>, query: Query<QueryParams>) -> actix_web::Result<Json<ViewResponse>> {
    let mut prefix = path.into_inner();
    if !prefix.is_empty() && !prefix.ends_with('/') {
        prefix.push('/');
    }
    let limit = MAX_LIMIT.or(query.limit);
    let cursor = query.cursor.clone();

    let cache_key = format!("s3://{bucket}/{path}@{cursor}+{limit}", bucket = S3_BUCKET.name(), path = prefix, cursor = cursor.clone().unwrap_or("".to_string()), limit = limit.unwrap_or(0));

    if let Some(cached) = cache_get::<ViewResponse>(cache_key.as_str()).await.unwrap_or(None) {
        return Ok(Json(cached));
    }

    debug!("Listing s3://{}/{} with cursor {:?} and limit {:?}", S3_BUCKET.name(), prefix, cursor, limit);

    let (result, status) = S3_BUCKET.list_page(prefix.clone(), Some(LISTOBJECTS_DELIMITER.to_string()), cursor.clone(), None, limit).await.map_err(|e| {
        error!("Error listing s3://{}/{}: {}", S3_BUCKET.name(), prefix, e);
        ErrorInternalServerError("Error listing S3 bucket")
    })?;
    if status < 200 || status >= 300 {
        error!("Error listing s3://{}/{}: Received HTTP status {}", S3_BUCKET.name(), prefix, status);
        return Err(ErrorInternalServerError("Error listing S3 bucket"));
    }

    let files = Some(result.contents.iter().filter_map(|obj| {
        obj.key.strip_prefix(&prefix).map(|filename| {
            let mut url_str = API_URL.clone();
            if !url_str.ends_with('/') {
                url_str.push('/');
            }
            let url = Url::parse(url_str.as_str()).expect("Invalid API_URL");
            let joined = url.join(format!("api/download/{}", obj.key).as_str()).expect("Invalid API_URL");

            let last_modified = DateTime::parse_from_rfc3339(obj.last_modified.as_str()).expect(format!("Invalid last_modified: {}", obj.last_modified).as_str()).to_utc();

            ViewFileResponse {
                name: filename.to_string(),
                last_modified,
                size: obj.size,
                download_url: Some(joined.to_string()),
            }
        })
    }).collect::<Vec<ViewFileResponse>>()).filter(|vec| !vec.is_empty());

    let folders = result.common_prefixes.map(|prefixes| {
        prefixes.iter().filter_map(|obj| {
            obj.prefix.strip_prefix(&prefix).map(|s| s.trim_end_matches('/').to_string())
        }).collect::<Vec<String>>()
    }).filter(|vec| !vec.is_empty());

    if files.is_none() && folders.is_none() {
        return Err(actix_web::error::ErrorNotFound("Not Found"));
    }

    let response = ViewResponse {
        path: prefix,
        files,
        folders,
        next_cursor: result.next_continuation_token,
    };
    cache_set(cache_key.as_str(), response.clone()).ok();

    Ok(Json(response))
}

#[get("/download/{path:.*}")]
pub async fn download_s3(path: Path<String>) -> actix_web::Result<Redirect> {
    let key = path.into_inner();

    let (_, status) = S3_BUCKET.head_object(&key).await.map_err(|e| {
        error!("Error listing s3://{}/{}: {}", S3_BUCKET.name(), key, e);
        ErrorInternalServerError("Error accessing S3 object")
    })?;

    if status < 200 || status >= 300 {
        return Err(actix_web::error::ErrorNotFound("Not Found"));
    }

    let url = S3_BUCKET.presign_get(&key, EXPIRY_TIME, None).await.map_err(|e| {
        error!("Error generating presigned URL for s3://{}/{}: {}", S3_BUCKET.name(), key, e);
        ErrorInternalServerError("Unable to generate download URL")
    })?;

    Ok(Redirect::to(url).temporary())
}

#[put("/upload/{path:.*}")]
pub async fn upload_s3(path: Path<String>, mut payload: Multipart) -> actix_web::Result<impl Responder> {
    let key = path.into_inner();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let cd = field.content_disposition();

        if cd.is_form_data() && cd.get_name().unwrap() == "file" {

            let content_type = field.content_type().map(|mt| mt.essence_str()).clone().unwrap_or("text/plain").to_string();

            let result = S3_BUCKET.initiate_multipart_upload(&key, content_type.as_str()).await.map_err(|e| {
                error!("Error initiating multipart upload for s3://{}/{}: {}", S3_BUCKET.name(), key, e);
                ErrorInternalServerError("Error initiating multipart upload")
            })?;

            let mut parts: Vec<Part> = Vec::new();
            while let Some(chunk) = field.next().await {
                if parts.len() >= 10000 {
                    error!("Too many parts for s3://{}/{}", S3_BUCKET.name(), key);
                    S3_BUCKET.abort_upload(result.key.as_str(), result.upload_id.as_str()).await.map_err(|e| {
                        error!("Error aborting upload for s3://{}/{}: {}", S3_BUCKET.name(), key, e);
                    }).ok();
                    return Err(ErrorInternalServerError("Too many parts"));
                }
                match chunk {
                    Ok(data) => {
                        let part = S3_BUCKET.put_multipart_chunk(data.to_vec(), result.key.as_str(), parts.len() as u32 + 1, result.upload_id.as_str(), content_type.as_str()).await.map_err(|e| {
                            error!("Error uploading s3://{}/{}: {}", S3_BUCKET.name(), key, e);
                            ErrorInternalServerError("Error uploading to S3")
                        })?;
                        parts.push(part);
                        Ok(())
                    }
                    Err(e) => {
                        error!("Error reading chunk: {e}");
                        S3_BUCKET.abort_upload(result.key.as_str(), result.upload_id.as_str()).await.map_err(|e| {
                            error!("Error aborting upload for s3://{}/{}: {}", S3_BUCKET.name(), key, e);
                        }).ok();
                        Err(ErrorInternalServerError("Error reading chunk"))
                    }
                }?;
            }

            S3_BUCKET.complete_multipart_upload(result.key.as_str(), result.upload_id.as_str(), parts).await.map_err(|e| {
                error!("Error completing multipart upload for s3://{}/{}: {}", S3_BUCKET.name(), key, e);
                ErrorInternalServerError("Error completing multipart upload")
            })?;

            cache_clear().await.ok();
            info!("Uploaded s3://{}/{}", S3_BUCKET.name(), key);

            return  Ok(HttpResponse::NoContent());
        }
    }

    Err(ErrorBadRequest("No 'file' field found in request"))
}