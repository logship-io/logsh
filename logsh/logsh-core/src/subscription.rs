use crate::{error::{SubscriptionError, self}, logship_client::LogshClientHandler};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionsModel {
    pub permissions: Vec<String>,
    pub account_id: uuid::Uuid,
    pub account_name: String,
}

pub fn list_subscriptions(
    connection : &LogshClientHandler,
    user_id : uuid::Uuid,
    include_all_if_admin : bool) -> Result<Vec<SubscriptionsModel>, SubscriptionError> {
    let query_url = format!("users/{}/accounts?allIfAdmin={}", user_id, include_all_if_admin);

    let result = connection.execute_func(&|client| -> Result<Vec<SubscriptionsModel>, error::ClientError> {
        let result = client.get_json(&query_url)?;
        Ok(result)
    })?;

    Ok(result)
}

pub fn delete_subscription(
    connection : &LogshClientHandler,
    subscription_id : uuid::Uuid) -> Result<(), SubscriptionError> {
    let query_url = format!("accounts/{}", subscription_id);

    let result = connection.execute_func(&|client| -> Result<(), error::ClientError> {
        let _result = client.delete(&query_url)?;
        Ok(())
    })?;

    Ok(result)
}