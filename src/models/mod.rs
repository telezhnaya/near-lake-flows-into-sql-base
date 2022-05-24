use bigdecimal::BigDecimal;
use futures::future::try_join_all;
use near_indexer_primitives::views::{
    AccessKeyPermissionView, ExecutionStatusView, StateChangeCauseView,
};

use futures::try_join;
use num_traits::{ToPrimitive, Zero};
use sqlx::{Arguments, Row};

pub use account_changes::AccountChange;
pub use execution_outcomes::{ExecutionOutcome, ExecutionOutcomeReceipt};
pub use near_lake_flows_into_sql::FieldCount;
pub use receipts::{ActionReceipt, ActionReceiptAction, ActionReceiptsOutput, DataReceipt};
pub use transactions::Transaction;

use crate::models::blocks::Block;
use crate::models::chunks::Chunk;
pub(crate) use serializers::extract_action_type_and_value_from_action_view;

pub(crate) mod account_changes;
pub(crate) mod blocks;
pub(crate) mod chunks;
pub(crate) mod execution_outcomes;
pub(crate) mod receipts;
mod serializers;
pub(crate) mod transactions;

pub trait FieldCount {
    /// Get the number of fields on a struct.
    fn field_count() -> usize;
}

pub trait MySqlMethods {
    fn add_to_args(&self, args: &mut sqlx::postgres::PgArguments);

    fn insert_query(count: usize) -> anyhow::Result<String>;

    fn delete_query() -> String;

    fn name() -> String;
}

pub async fn chunked_insert<T: MySqlMethods + std::fmt::Debug>(
    pool: &sqlx::Pool<sqlx::Postgres>,
    items: &[T],
    retry_count: usize,
) -> anyhow::Result<()> {
    let futures = items
        .chunks(crate::db_adapters::CHUNK_SIZE_FOR_BATCH_INSERT)
        .map(|items_part| insert_retry_or_panic(pool, items_part, retry_count));
    try_join_all(futures).await.map(|_| ())
}

async fn insert_retry_or_panic<T: MySqlMethods + std::fmt::Debug>(
    pool: &sqlx::Pool<sqlx::Postgres>,
    items: &[T],
    retry_count: usize,
) -> anyhow::Result<()> {
    let mut interval = crate::INTERVAL;
    let mut retry_attempt = 0usize;
    let query = T::insert_query(items.len())?;

    loop {
        if retry_attempt == retry_count {
            return Err(anyhow::anyhow!(
                "Failed to perform query to database after {} attempts. Stop trying.",
                retry_count
            ));
        }
        retry_attempt += 1;

        let mut args = sqlx::postgres::PgArguments::default();
        for item in items {
            item.add_to_args(&mut args);
        }

        match sqlx::query_with(&query, args).execute(pool).await {
            Ok(_) => break,
            Err(async_error) => {
                tracing::error!(
                         target: crate::INDEXER,
                         "Error occurred during {}:\n{} were not stored. \n{:#?} \n Retrying in {} milliseconds...",
                         async_error,
                         &T::name(),
                         &items,
                         interval.as_millis(),
                     );
                tokio::time::sleep(interval).await;
                if interval < crate::MAX_DELAY_TIME {
                    interval *= 2;
                }
            }
        }
    }
    Ok(())
}

pub async fn select_retry_or_panic(
    pool: &sqlx::Pool<sqlx::Postgres>,
    query: &str,
    substitution_items: &[String],
    retry_count: usize,
) -> anyhow::Result<Vec<sqlx::postgres::PgRow>> {
    let mut interval = crate::INTERVAL;
    let mut retry_attempt = 0usize;

    loop {
        if retry_attempt == retry_count {
            return Err(anyhow::anyhow!(
                "Failed to perform query to database after {} attempts. Stop trying.",
                retry_count
            ));
        }
        retry_attempt += 1;

        let mut args = sqlx::postgres::PgArguments::default();
        for item in substitution_items {
            args.add(item);
        }

        match sqlx::query_with(query, args).fetch_all(pool).await {
            Ok(res) => return Ok(res),
            Err(async_error) => {
                // todo we print here select with non-filled placeholders. It would be better to get the final select statement here
                tracing::error!(
                         target: crate::INDEXER,
                         "Error occurred during {}:\nFailed SELECT:\n{}\n Retrying in {} milliseconds...",
                         async_error,
                    query,
                         interval.as_millis(),
                     );
                tokio::time::sleep(interval).await;
                if interval < crate::MAX_DELAY_TIME {
                    interval *= 2;
                }
            }
        }
    }
}

async fn delete_retry_or_panic<T: MySqlMethods + std::fmt::Debug>(
    pool: &sqlx::Pool<sqlx::Postgres>,
    timestamp: &BigDecimal,
    retry_count: usize,
) -> anyhow::Result<()> {
    let mut interval = crate::INTERVAL;
    let mut retry_attempt = 0usize;
    let query = T::delete_query();

    loop {
        if retry_attempt == retry_count {
            return Err(anyhow::anyhow!(
                "Failed to perform query to database after {} attempts. Stop trying.",
                retry_count
            ));
        }
        retry_attempt += 1;

        let mut args = sqlx::postgres::PgArguments::default();
        args.add(timestamp);

        match sqlx::query_with(&query, args).execute(pool).await {
            Ok(_) => break,
            Err(async_error) => {
                tracing::error!(
                         target: crate::INDEXER,
                         "Error occurred during {}:\n could not clean up {}. \n Retrying in {} milliseconds...",
                         async_error,
                         &T::name(),
                         interval.as_millis(),
                     );
                tokio::time::sleep(interval).await;
                if interval < crate::MAX_DELAY_TIME {
                    interval *= 2;
                }
            }
        }
    }
    Ok(())
}

// Generates `($1, $2), ($3, $4)`
pub(crate) fn create_placeholders(
    mut items_count: usize,
    fields_count: usize,
) -> anyhow::Result<String> {
    if items_count < 1 {
        return Err(anyhow::anyhow!("At least 1 item expected"));
    }

    let mut start_num: usize = 1;
    let mut res = create_placeholder(&mut start_num, fields_count)?;
    items_count -= 1;
    while items_count > 0 {
        let placeholder = create_placeholder(&mut start_num, fields_count)?;
        res += ", ";
        res += &placeholder;
        items_count -= 1;
    }

    Ok(res)
}

// Generates `($1, $2, $3)`
pub(crate) fn create_placeholder(
    start_num: &mut usize,
    mut fields_count: usize,
) -> anyhow::Result<String> {
    if fields_count < 1 {
        return Err(anyhow::anyhow!("At least 1 field expected"));
    }
    let mut item = format!("(${}", start_num);
    *start_num += 1;
    fields_count -= 1;
    while fields_count > 0 {
        item += &format!(", ${}", start_num);
        *start_num += 1;
        fields_count -= 1;
    }
    item += ")";
    Ok(item)
}

pub(crate) async fn start_after_interruption(
    pool: &sqlx::Pool<sqlx::Postgres>,
) -> anyhow::Result<u64> {
    let query = "SELECT block_timestamp
                        FROM blocks
                        ORDER BY block_timestamp desc
                        LIMIT 1";

    let res = select_retry_or_panic(pool, query, &[], 10).await?;
    let timestamp: BigDecimal = res
        .first()
        .map(|value| value.get(0))
        .unwrap_or_else(BigDecimal::zero);

    try_join!(
        delete_retry_or_panic::<AccountChange>(pool, &timestamp, 10),
        delete_retry_or_panic::<Block>(pool, &timestamp, 10),
        delete_retry_or_panic::<Chunk>(pool, &timestamp, 10),
        delete_retry_or_panic::<ExecutionOutcome>(pool, &timestamp, 10),
        delete_retry_or_panic::<ExecutionOutcomeReceipt>(pool, &timestamp, 10),
        delete_retry_or_panic::<ActionReceipt>(pool, &timestamp, 10),
        delete_retry_or_panic::<DataReceipt>(pool, &timestamp, 10),
        delete_retry_or_panic::<ActionReceiptAction>(pool, &timestamp, 10),
        delete_retry_or_panic::<ActionReceiptsOutput>(pool, &timestamp, 10),
        delete_retry_or_panic::<Transaction>(pool, &timestamp, 10)
    )?;

    let query = "SELECT block_height
                        FROM blocks
                        ORDER BY block_timestamp desc
                        LIMIT 1";

    let res = select_retry_or_panic(pool, query, &[], 10).await?;
    let height: u64 = res
        .first()
        .map(|value| value.get(0))
        .unwrap_or_else(BigDecimal::zero)
        .to_u64()
        .expect("height should be positive");
    Ok(height + 1)
}

pub(crate) trait PrintEnum {
    fn print(&self) -> &str;
}

impl PrintEnum for ExecutionStatusView {
    fn print(&self) -> &str {
        match self {
            ExecutionStatusView::Unknown => "UNKNOWN",
            ExecutionStatusView::Failure(_) => "FAILURE",
            ExecutionStatusView::SuccessValue(_) => "SUCCESS_VALUE",
            ExecutionStatusView::SuccessReceiptId(_) => "SUCCESS_RECEIPT_ID",
        }
    }
}

impl PrintEnum for AccessKeyPermissionView {
    fn print(&self) -> &str {
        match self {
            AccessKeyPermissionView::FunctionCall { .. } => "FUNCTION_CALL",
            AccessKeyPermissionView::FullAccess => "FULL_ACCESS",
        }
    }
}

impl PrintEnum for StateChangeCauseView {
    fn print(&self) -> &str {
        match self {
            StateChangeCauseView::NotWritableToDisk => {
                panic!("Unexpected variant {:?} received", self)
            }
            StateChangeCauseView::InitialState => panic!("Unexpected variant {:?} received", self),
            StateChangeCauseView::TransactionProcessing { .. } => "TRANSACTION_PROCESSING",
            StateChangeCauseView::ActionReceiptProcessingStarted { .. } => {
                "ACTION_RECEIPT_PROCESSING_STARTED"
            }
            StateChangeCauseView::ActionReceiptGasReward { .. } => "ACTION_RECEIPT_GAS_REWARD",
            StateChangeCauseView::ReceiptProcessing { .. } => "RECEIPT_PROCESSING",
            StateChangeCauseView::PostponedReceipt { .. } => "POSTPONED_RECEIPT",
            StateChangeCauseView::UpdatedDelayedReceipts => "UPDATED_DELAYED_RECEIPTS",
            StateChangeCauseView::ValidatorAccountsUpdate => "VALIDATOR_ACCOUNTS_UPDATE",
            StateChangeCauseView::Migration => "MIGRATION",
            StateChangeCauseView::Resharding => "RESHARDING",
        }
    }
}
