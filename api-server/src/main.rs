mod db;
mod schema;
use anyhow::{bail, Context};
use axum::extract::multipart::Field;
use axum::extract::DefaultBodyLimit;
use axum::extract::Multipart;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{
    routing::{get, post},
    Router,
};
use db::{
    establish_connection, find_file_by_file_name, find_file_by_file_type,
    find_file_by_file_upload_date, insert_file, list_file_names,
};
use diesel::SqliteConnection;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs::{create_dir_all, File};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize, Debug)]
struct FileUploadRequest {
    pub file_name: String,
    pub file_type: Option<String>,
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
        create_dir_all(parent).await?;
    }
    let mut file = File::create(path).await?;
    while let Some(bytes) = file_field.next().await {
        let bytes = bytes?;
        file.write_all(&bytes).await?;
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

async fn accept_file_stream(
    db: State<Arc<Mutex<SqliteConnection>>>,
    data: Multipart,
) -> Result<impl IntoResponse, StatusCode> {
    let result = process_file_stream(data).await;
    match result {
        Ok(response) => {
            let file = db::File {
                file_name: response.file_name,
                file_type: response.file_type,
                file_upload_date: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32,
            };
            if let Err(e) = insert_file(db.0, &file).await {
                eprintln!("{:?}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Ok(format!("{:?}", file))
        }
        Err(e) => {
            eprintln!("{:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn list_files(
    db: State<Arc<Mutex<SqliteConnection>>>,
) -> Result<impl IntoResponse, StatusCode> {
    let files = list_file_names(db.0).await;
    match files {
        Ok(files) => match serde_json::to_string(&files) {
            Ok(json_str) => Ok(json_str),
            Err(e) => {
                eprintln!("{:?}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        },
        Err(e) => {
            eprintln!("{:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
struct FileFilterAttributes {
    file_name: Option<String>,
    file_type: Option<String>,
    file_upload_date: Option<i32>,
}

async fn filter_files(
    db: State<Arc<Mutex<SqliteConnection>>>,
    Query(attributes): Query<FileFilterAttributes>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut results = Vec::<std::collections::BTreeSet<String>>::new();
    if let Some(ref file_name) = attributes.file_name {
        match find_file_by_file_name(db.0.clone(), file_name).await {
            Ok(files) => {
                results.push(files.into_iter().map(|file| file.file_name).collect());
            }
            Err(e) => {
                eprintln!("{:?}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }
    if let Some(ref file_type) = attributes.file_type {
        match find_file_by_file_type(db.0.clone(), file_type).await {
            Ok(files) => {
                results.push(files.into_iter().map(|file| file.file_name).collect());
            }
            Err(e) => {
                eprintln!("{:?}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }
    if let Some(ref file_upload_date) = attributes.file_upload_date {
        match find_file_by_file_upload_date(db.0.clone(), file_upload_date).await {
            Ok(files) => {
                results.push(files.into_iter().map(|file| file.file_name).collect());
            }
            Err(e) => {
                eprintln!("{:?}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }
    while results.len() > 1 {
        let set_a = results.pop().unwrap();
        let set_b = results.pop().unwrap();
        results.push(set_a.intersection(&set_b).map(|s| s.to_owned()).collect());
    }
    let result: Vec<String> = if results.len() == 1 {
        results.pop().unwrap().into_iter().collect()
    } else {
        vec![]
    };
    match serde_json::to_string(&result) {
        Ok(json_str) => Ok(json_str),
        Err(e) => {
            eprintln!("{:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[tokio::main]
async fn main() {
    let db = Arc::new(Mutex::new(establish_connection()));
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/audio", get(list_files).post(accept_file_stream))
        .route("/audio/query", get(filter_files))
        .with_state(db)
        .layer(DefaultBodyLimit::disable());
    axum::Server::bind(&"127.0.0.1:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
