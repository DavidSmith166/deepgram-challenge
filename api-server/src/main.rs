use anyhow::{anyhow, bail, Context};
use axum::body::Body;
use axum::body::Bytes;
use axum::extract::multipart::Field;
use axum::extract::DefaultBodyLimit;
use axum::extract::Multipart;
use axum::extract::{BodyStream, Json};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{
    routing::{get, post},
    Router,
};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::format;
use std::fs::{self, Metadata};
use std::io::prelude::*;
use std::path::Path;
use std::string;

#[derive(Serialize, Deserialize, Debug)]
struct FileUploadRequest {
    file_name: String,
    file_type: Option<String>,
}

async fn write_file<'a>(
    upload_request: &FileUploadRequest,
    mut file_field: Field<'a>,
) -> Result<(), anyhow::Error> {
    let mut path = std::env::current_dir()?;
    path.push("audio");
    path.push(&upload_request.file_name);
    println!("writing file to path: {:?}", path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(path)?;
    if let Some(ref file_type) = upload_request.file_type {
        match file_type.as_str() {
            "wav" => {
                // todo extract metadata from wav
                while let Some(bytes) = file_field.next().await {
                    let bytes = bytes?;
                    file.write_all(&bytes)?;
                }
            }
            _ => todo!(),
        }
    }
    Ok(())
}

async fn process_file_stream(mut data: Multipart) -> Result<FileUploadRequest, anyhow::Error> {
    let mut fields = BTreeMap::<String, Value>::new();
    let file_field = loop {
        if let Some(field) = data.next_field().await? {
            let name = field.name().context("missing field name")?.to_owned();
            if name == "file" {
                break field;
            }
            let data = field.bytes().await?;
            fields.insert(name, std::str::from_utf8(&data)?.to_owned().into());
        } else {
            bail!("File upload ended early");
        }
    };
    let json = serde_json::to_string(&fields)?;
    let upload_request = serde_json::from_str::<FileUploadRequest>(&json)?;
    write_file(&upload_request, file_field).await?;
    Ok(upload_request)
}

async fn accept_file_stream(data: Multipart) -> Result<impl IntoResponse, StatusCode> {
    let result = process_file_stream(data).await;
    match result {
        Ok(response) => Ok(format!("{:?}", response)),
        Err(e) => {
            eprintln!("{:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/audio", post(accept_file_stream))
        .layer(DefaultBodyLimit::disable());
    // run it with hyper on localhost:3000
    axum::Server::bind(&"127.0.0.1:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
