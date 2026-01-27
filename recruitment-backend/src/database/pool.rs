use crate::config::get_config;
use crate::error::Result;
use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn create_pool() -> Result<PgPool> {
    let config = get_config();
    let pool = PgPoolOptions::new()
        .max_connections(50)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(&config.database_url)
        .await?;
    Ok(pool)
}
