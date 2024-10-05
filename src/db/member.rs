use sqlx::FromRow;
use serde::Deserialize;

#[derive(FromRow,Clone, Deserialize)]
pub struct Member {
    pub name: String,
    pub id: i32,
}