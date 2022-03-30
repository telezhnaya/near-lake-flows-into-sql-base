use near_indexer_primitives::views::{
    AccessKeyPermissionView, ExecutionStatusView, ReceiptEnumView, StateChangeCauseView,
};

pub use near_lake_flows_into_sql::FieldCount;
pub use receipts::{
    ActionReceipt, ActionReceiptAction, ActionReceiptInputData, ActionReceiptOutputData,
    DataReceipt, Receipt,
};
pub use transactions::Transaction;

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

fn create_query_with_placeholders(
    query: &str,
    mut items_count: usize,
    mut fields_count: usize,
) -> anyhow::Result<String> {
    if items_count < 1 || fields_count < 1 {
        return Err(anyhow::anyhow!(
            "At least 1 item expected with at least 1 field inside"
        ));
    }

    // Generating `(?, ?, ?)`
    let mut item = "(?".to_owned();
    fields_count -= 1;
    while fields_count > 0 {
        item += ", ?";
        fields_count -= 1;
    }
    item += ")";

    // Generating `INSERT INTO table VALUES (?, ?, ?), (?, ?, ?)`
    let mut res = query.to_owned() + " " + &item;
    items_count -= 1;
    while items_count > 0 {
        res += ", ";
        res += &item;
        items_count -= 1;
    }

    Ok(res)
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

impl PrintEnum for ReceiptEnumView {
    fn print(&self) -> &str {
        match self {
            ReceiptEnumView::Action { .. } => "ACTION",
            ReceiptEnumView::Data { .. } => "DATA",
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
