use bigdecimal::BigDecimal;
use dotenv::dotenv;
use futures::StreamExt;
use near_indexer_primitives;
use near_lake_framework::LakeConfig;
use num_traits::cast::FromPrimitive;
use serde::{Deserialize, Serialize};
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::{FromRow, MySql, MySqlPool, Pool, Row};
use std::convert::TryFrom;
use std::env;
use std::str::FromStr;
use tracing_subscriber::EnvFilter;

#[derive(Debug, FromRow, Serialize, Deserialize)]
struct Aaa {
    // https://docs.rs/sqlx/0.4.0-beta.1/sqlx/mysql/types/index.html
    a: BigDecimal,
}

#[derive(Debug, FromRow)]
pub struct Block {
    pub block_height: BigDecimal,
    pub block_hash: String,
    pub prev_block_hash: String,
    pub block_timestamp: BigDecimal,
    pub total_supply: BigDecimal,
    pub gas_price: BigDecimal,
    pub author_account_id: String,
}

impl From<&near_indexer_primitives::views::BlockView> for Block {
    fn from(block_view: &near_indexer_primitives::views::BlockView) -> Self {
        Self {
            block_height: block_view.header.height.into(),
            block_hash: block_view.header.hash.to_string(),
            prev_block_hash: block_view.header.prev_hash.to_string(),
            block_timestamp: block_view.header.timestamp.into(),
            total_supply: BigDecimal::from_str(block_view.header.total_supply.to_string().as_str())
                .expect("`total_supply` expected to be u128"),
            gas_price: BigDecimal::from_str(block_view.header.gas_price.to_string().as_str())
                .expect("`gas_price` expected to be u128"),
            author_account_id: block_view.author.to_string(),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let pool = MySqlPool::connect(&env::var("DATABASE_URL")?).await?;

    let select_query = sqlx::query_as::<MySql, Aaa>("SELECT * FROM aaa");
    // let select_query = sqlx::query_as!(Aaa, "SELECT * FROM aaa");
    let a = select_query.fetch_all(&pool).await?;

    init_tracing();

    let config = LakeConfig {
        s3_bucket_name: "near-lake-testnet".to_string(),
        s3_region_name: "eu-central-1".to_string(),
        start_block_height: 83030086, // want to start from the freshest
    };
    let stream = near_lake_framework::streamer(config);

    let mut handlers = tokio_stream::wrappers::ReceiverStream::new(stream)
        .map(|streamer_message| handle_streamer_message(streamer_message, &pool))
        .buffer_unordered(1usize);

    while let Some(_handle_message) = handlers.next().await {}

    Ok(())
}

// TODO check that I don't need to adapt the types (it's created successfully)
// TODO think about migrations
// CREATE TABLE blocks (
//     block_height numeric(20,0) NOT NULL,
//     block_hash text NOT NULL,
//     prev_block_hash text NOT NULL,
//     block_timestamp numeric(20,0) NOT NULL,
//     total_supply numeric(45,0) NOT NULL,
//     gas_price numeric(45,0) NOT NULL,
//     author_account_id text NOT NULL,
//     PRIMARY KEY (block_hash)
// );

async fn handle_streamer_message(
    streamer_message: near_lake_framework::near_indexer_primitives::StreamerMessage,
    pool: &Pool<MySql>,
) {
    let block_model = Block::from(&streamer_message.block);
    eprintln!(
        "{} / shards {}",
        streamer_message.block.header.height,
        streamer_message.shards.len()
    );
    // TODO find a better way to insert the objects to the DB
    let new_user = sqlx::query!(
        r#"
       INSERT INTO blocks
       VALUES (?, ?, ?, ?, ?, ?, ?)
       "#,
        block_model.block_height,
        block_model.block_hash,
        block_model.prev_block_hash,
        block_model.block_timestamp,
        block_model.total_supply,
        block_model.gas_price,
        block_model.author_account_id
    );
    let a = new_user.fetch_all(&pool.clone()).await;
}

fn init_tracing() {
    let mut env_filter = EnvFilter::new("near_lake_framework=info");

    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        if !rust_log.is_empty() {
            for directive in rust_log.split(',').filter_map(|s| match s.parse() {
                Ok(directive) => Some(directive),
                Err(err) => {
                    eprintln!("Ignoring directive `{}`: {}", s, err);
                    None
                }
            }) {
                env_filter = env_filter.add_directive(directive);
            }
        }
    }

    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .init();
}
