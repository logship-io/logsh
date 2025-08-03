use crate::{error::{AccountError, self}, logship_client::LogshClientHandler};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsModel {
    pub permissions: Vec<String>,
    pub account_id: uuid::Uuid,
    pub account_name: String,
}

pub fn list_accounts(
    connection : &LogshClientHandler,
    user_id : uuid::Uuid,
    include_all_if_admin : bool) -> Result<Vec<AccountsModel>, AccountError> {
    let query_url = format!("users/{}/accounts?allIfAdmin={}", user_id, include_all_if_admin);

    let result = connection.execute_func(&|client| -> Result<Vec<AccountsModel>, error::ClientError> {
        let result = client.get_json(&query_url)?;
        Ok(result)
    })?;

    Ok(result)
}

pub fn delete_account(
    connection : &LogshClientHandler,
    account_id : uuid::Uuid) -> Result<(), AccountError> {
    let query_url = format!("accounts/{}", account_id);

    let result = connection.execute_func(&|client| -> Result<(), error::ClientError> {
        client.delete(&query_url)?;
        Ok(())
    })?;

    Ok(result)
}