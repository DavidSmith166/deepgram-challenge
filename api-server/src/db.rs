use crate::schema::files;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Queryable, Insertable, Serialize, Deserialize, Debug, PartialEq)]
#[diesel(table_name = files)]
#[diesel(treat_none_as_default_value = false)]
pub struct File {
    pub file_name: String,
    pub file_type: Option<String>,
    pub file_upload_date: i32,
}

// Right now all of these functions block async threads because diesel predates tokio
// Todo: use tokio_diesel

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub async fn insert_file(
    conn: Arc<Mutex<SqliteConnection>>,
    file: &File,
) -> Result<(), anyhow::Error> {
    file.insert_into(files::table)
        .execute(&mut *conn.lock().await)?;
    Ok(())
}

pub async fn list_file_names(
    conn: Arc<Mutex<SqliteConnection>>,
) -> Result<Vec<String>, anyhow::Error> {
    use super::schema::files::dsl::*;
    Ok(files
        .select(file_name)
        .load::<String>(&mut *conn.lock().await)?)
}

pub async fn find_file_by_file_name(
    conn: Arc<Mutex<SqliteConnection>>,
    target: &str,
) -> Result<Vec<File>, anyhow::Error> {
    use super::schema::files::dsl::*;
    Ok(files
        .filter(file_name.eq(target))
        .load::<File>(&mut *conn.lock().await)?)
}

pub async fn find_file_by_file_type(
    conn: Arc<Mutex<SqliteConnection>>,
    target: &str,
) -> Result<Vec<File>, anyhow::Error> {
    use super::schema::files::dsl::*;
    Ok(files
        .filter(file_type.eq(target))
        .load::<File>(&mut *conn.lock().await)?)
}

pub async fn find_file_by_file_upload_date(
    conn: Arc<Mutex<SqliteConnection>>,
    target: &i32,
) -> Result<Vec<File>, anyhow::Error> {
    use super::schema::files::dsl::*;
    Ok(files
        .filter(file_upload_date.eq(target))
        .load::<File>(&mut *conn.lock().await)?)
}
