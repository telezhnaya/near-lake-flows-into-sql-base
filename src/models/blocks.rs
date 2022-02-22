use std::fmt;
use std::str::FromStr;

use bigdecimal::BigDecimal;

#[derive(Debug, sqlx::FromRow)]
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

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "('{}','{}','{}','{}','{}','{}','{}')",
            self.block_height,
            self.block_hash,
            self.prev_block_hash,
            self.block_timestamp,
            self.total_supply,
            self.gas_price,
            self.author_account_id
        )
    }
}
