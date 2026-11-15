use crate::{
    error::{self, AccountError},
    logship_client::LogshClientHandler,
};
use serde::{Deserialize, Serialize};

pub fn resolve_account<'a>(
    accounts: &'a [AccountsModel],
    selector: &str,
) -> Result<&'a AccountsModel, crate::error::ConnectError> {
    let id = uuid::Uuid::parse_str(selector).ok();
    let matches: Vec<_> = accounts
        .iter()
        .filter(|a| {
            id.map_or_else(
                || a.account_name.eq_ignore_ascii_case(selector),
                |id| a.account_id == id,
            )
        })
        .collect();
    match matches.as_slice() {
        [account] => Ok(account),
        [] => Err(crate::error::ConnectError::InvalidConfigError(format!(
            "Account '{selector}' is not accessible. Run logsh account list."
        ))),
        _ => Err(crate::error::ConnectError::InvalidConfigError(format!(
            "Account name '{selector}' is ambiguous. Use an account UUID."
        ))),
    }
}

/// Model representing a user's account with its permissions.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsModel {
    pub permissions: Vec<String>,
    pub account_id: uuid::Uuid,
    pub account_name: String,
}

/// Lists accounts accessible by the given user, optionally including all accounts for admins.
pub fn list_accounts(
    connection: &LogshClientHandler,
    user_id: uuid::Uuid,
    include_all_if_admin: bool,
) -> Result<Vec<AccountsModel>, AccountError> {
    let query_url = format!("users/{user_id}/accounts?allIfAdmin={include_all_if_admin}");

    let result = connection.execute_func(&|client| -> Result<
        Vec<AccountsModel>,
        error::ClientError,
    > {
        let result = client.get_json(&query_url)?;
        Ok(result)
    })?;

    Ok(result)
}

/// Deletes the account with the given ID.
pub fn delete_account(
    connection: &LogshClientHandler,
    account_id: uuid::Uuid,
) -> Result<(), AccountError> {
    let query_url = format!("accounts/{account_id}");

    connection.execute_func(&|client| -> Result<(), error::ClientError> {
        client.delete(&query_url)?;
        Ok(())
    })?;

    Ok(())
}
