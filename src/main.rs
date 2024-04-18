use std::env;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;

use actix_cors::Cors;
use actix_files::{Files, NamedFile};
use actix_web::{App, HttpServer, web};
use actix_web::dev::{fn_service, ServiceRequest, ServiceResponse};
use log::warn;
use once_cell::sync::Lazy;
use s3::{Bucket, Region};
use s3::creds::Credentials;

use crate::s3_route::{download_s3, list_s3};

mod s3_route;
mod cache;

pub mod build_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

const VERSION: Lazy<&'static str> = Lazy::new(|| option_env!("VERSION").unwrap_or(build_info::PKG_VERSION));
const ENVIRONMENT: Lazy<&'static str> = Lazy::new(|| option_env!("RUST_ENVIRONMENT").unwrap_or("development"));

const FRONTEND_PATH: Lazy<Option<PathBuf>> = Lazy::new(|| env::var("FRONTEND_PATH").ok().map(|path| PathBuf::from(path).canonicalize().expect("Unable to canonicalize FRONTEND_PATH")));
const API_URL: Lazy<String> = Lazy::new(|| env::var("API_URL").unwrap_or("http://localhost:3001".to_string()));

const S3_BUCKET: Lazy<Bucket> = Lazy::new(|| {
    let s3_region = Region::from_env("S3_REGION", Some("S3_ENDPOINT")).expect("S3_REGION and/or S3_ENDPOINT must be defined");
    let s3_bucket = env::var("S3_BUCKET_NAME").expect("S3_BUCKET_NAME must be defined");
    let s3_credentials = Credentials::from_env_specific(
        Some("S3_ACCESS_KEY_ID"),
        Some("S3_SECRET_ACCESS_KEY"),
        Some("S3_SECURITY_TOKEN"),
        Some("S3_SESSION_TOKEN"),
    ).expect("Unable to parse S3 credentials");
    let use_path_style = env::var("S3_USE_PATH_STYLE").ok().map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false);
    let use_listobjects_v2 = env::var("S3_USE_LISTOBJECTS_V2").ok().map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(true);

    let mut bucket = Bucket::new(&s3_bucket, s3_region, s3_credentials).expect("Unable to create S3 bucket");
    if use_path_style {
        bucket.set_path_style();
    }
    if use_listobjects_v2 {
        bucket.set_listobjects_v2();
    }

    bucket
});

const REDIS: Lazy<Option<redis::Client>> = Lazy::new(|| {
    let redis_url = env::var("REDIS_URL").ok();
    match redis_url {
        Some(url) => {
            Some(redis::Client::open(url).expect("Unable to create Redis client"))
        }
        None => {
            log::info!("REDIS_URL is not defined. Redis caching will be disabled.");
            None
        }
    }
});

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    if ENVIRONMENT.eq("development") {
        env_logger::try_init_from_env(env_logger::Env::new().default_filter_or("info"))?;
    } else {
        tracing_subscriber::fmt().json().init();
    }
    log::info!("Starting S3 FileServer v{}", VERSION.clone());

    log::info!("Connected to {} at {}", S3_BUCKET.name(), S3_BUCKET.region().endpoint());

    if let Some(client) = REDIS.clone() {
        log::info!("Connected to Redis at {}", client.get_connection_info().addr);
    }

    log::info!("API URL: {}", API_URL.clone());
    if API_URL.eq("http://localhost:3001") {
        warn!("API_URL is not defined. Unless you are in a development environment, things are not going to work!");
    }

    HttpServer::new(|| {
        let cors = Cors::permissive();
        let app = App::new()
            .wrap(cors)
            .service(web::scope("/api")
                .service(list_s3)
                .service(download_s3)
            )
            .service(web::scope("/download")
                .service(list_s3)
            );

        if FRONTEND_PATH.is_some() {
            let files = Files::new("/", FRONTEND_PATH.clone().unwrap())
                .index_file("index.html")
                .default_handler(fn_service(|req: ServiceRequest| async {
                    let (req, _) = req.into_parts();
                    let file = NamedFile::open_async(FRONTEND_PATH.clone().unwrap().join("index.html")).await?;
                    let res = file.into_response(&req);
                    Ok(ServiceResponse::new(req, res))
                }))
                .prefer_utf8(true);

            return app.service(files);
        } else {
            return app;
        }
    })
        .bind((Ipv4Addr::UNSPECIFIED, 3001))?
        // FIXME this breaks in docker
        // .bind((Ipv6Addr::UNSPECIFIED, 3001))?
        .run()
        .await?;

    Ok(())
}
