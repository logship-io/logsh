//! Account administration models matching the backend OpenAPI contract.
use crate::{error::ClientError, logship_client::LogshClient};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountModel {
    pub account_id: uuid::Uuid,
    pub account_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicUserModel {
    pub user_id: uuid::Uuid,
    pub user_name: String,
    pub first_name: String,
    pub last_name: String,
    pub nick_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUserViewModel {
    pub user: BasicUserModel,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAccountViewModel {
    pub user_id: uuid::Uuid,
    pub permissions: Vec<String>,
    pub account_id: uuid::Uuid,
    pub account_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUserSchemaPermissions {
    pub schema_name: String,
    pub can_read: bool,
    pub can_write: bool,
    pub columns: Vec<ColumnLimitedAccessModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnLimitedAccessModel {
    pub column_name: String,
    pub can_read: bool,
    pub can_write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSettingInfo {
    pub setting: AccountSettingValue,
    pub description: String,
    pub is_sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSettingValue {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PutAccountSettingArgsModel {
    #[serde(default)]
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAccountPermissionsModel {
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAccountPermissionsByUsernameModel {
    pub user_name: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupApiModel {
    #[serde(default)]
    pub account_id: uuid::Uuid,
    #[serde(default)]
    pub group_id: uuid::Uuid,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created_by_user_id: uuid::Uuid,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub created_at: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMemberApiModel {
    #[serde(default)]
    pub account_id: uuid::Uuid,
    #[serde(default)]
    pub group_id: uuid::Uuid,
    #[serde(default)]
    pub user_id: uuid::Uuid,
    #[serde(default)]
    pub added_by_user_id: uuid::Uuid,
    #[serde(default)]
    pub added_at: String,
}

/// Account-scoped typed API. UUIDs are resolved before constructing this client.
pub struct AccountApi<'a> {
    pub client: &'a LogshClient,
    pub account_id: uuid::Uuid,
}
impl AccountApi<'_> {
    fn path(&self, suffix: &str) -> String {
        format!("accounts/{}{}", self.account_id, suffix)
    }
    fn get<T: for<'de> Deserialize<'de>>(&self, suffix: &str) -> Result<T, ClientError> {
        self.client.get_json(&self.path(suffix))
    }
    fn write<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        suffix: &str,
        body: &T,
    ) -> Result<R, ClientError> {
        serde_json::from_value(
            self.client
                .send_json(method, &self.path(suffix), Some(body))?,
        )
        .map_err(|e| ClientError::Common(crate::error::CommonError::Json(e)))
    }
    pub fn show(&self) -> Result<AccountModel, ClientError> {
        self.get("")
    }
    pub fn update(&self, name: String) -> Result<AccountModel, ClientError> {
        self.write(
            "PUT",
            "",
            &AccountModel {
                account_id: self.account_id,
                account_name: name,
            },
        )
    }
    pub fn permissions(&self) -> Result<Vec<String>, ClientError> {
        self.get("/permissions")
    }
    pub fn users(&self) -> Result<Vec<BasicUserModel>, ClientError> {
        self.get("/users")
    }
    pub fn user(&self, id: uuid::Uuid) -> Result<AccountUserViewModel, ClientError> {
        self.get(&format!("/users/{id}"))
    }
    pub fn add_user(
        &self,
        name: String,
        permissions: Vec<String>,
    ) -> Result<UserAccountViewModel, ClientError> {
        self.write(
            "POST",
            "/users",
            &SetAccountPermissionsByUsernameModel {
                user_name: name,
                permissions,
            },
        )
    }
    pub fn set_permissions(
        &self,
        id: uuid::Uuid,
        permissions: Vec<String>,
    ) -> Result<UserAccountViewModel, ClientError> {
        self.write(
            "POST",
            &format!("/users/{id}"),
            &SetAccountPermissionsModel { permissions },
        )
    }
    pub fn remove_user(&self, id: uuid::Uuid) -> Result<(), ClientError> {
        self.client.delete(&self.path(&format!("/users/{id}")))
    }
    pub fn schema_permissions(
        &self,
        id: uuid::Uuid,
    ) -> Result<Vec<AccountUserSchemaPermissions>, ClientError> {
        self.get(&format!("/users/{id}/schema-permissions"))
    }
    pub fn set_schema_permissions(
        &self,
        id: uuid::Uuid,
        permissions: &[AccountUserSchemaPermissions],
    ) -> Result<(), ClientError> {
        self.client.send_json(
            "PUT",
            &self.path(&format!("/users/{id}/schema-permissions")),
            Some(&permissions),
        )?;
        Ok(())
    }
    pub fn settings(&self) -> Result<Vec<AccountSettingInfo>, ClientError> {
        self.get("/settings")
    }
    pub fn set_setting(&self, name: &str, value: serde_json::Value) -> Result<(), ClientError> {
        // Encode the setting as a single URL path segment.
        let encoded: String = name
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect();
        self.client.send_json(
            "PUT",
            &self.path(&format!("/settings/{encoded}")),
            Some(&PutAccountSettingArgsModel { value }),
        )?;
        Ok(())
    }
    pub fn groups(&self) -> Result<Vec<GroupApiModel>, ClientError> {
        self.get("/groups")
    }
    pub fn put_group(&self, group: &GroupApiModel) -> Result<GroupApiModel, ClientError> {
        self.write("PUT", &format!("/groups/{}", group.group_id), group)
    }
    pub fn delete_group(&self, id: uuid::Uuid) -> Result<(), ClientError> {
        self.client.delete(&self.path(&format!("/groups/{id}")))
    }
    pub fn members(&self, id: uuid::Uuid) -> Result<Vec<GroupMemberApiModel>, ClientError> {
        self.get(&format!("/groups/{id}/members"))
    }
    pub fn add_member(
        &self,
        group: uuid::Uuid,
        user: uuid::Uuid,
    ) -> Result<GroupMemberApiModel, ClientError> {
        serde_json::from_value(self.client.send_json::<serde_json::Value>(
            "PUT",
            &self.path(&format!("/groups/{group}/members/{user}")),
            None,
        )?)
        .map_err(|e| ClientError::Common(crate::error::CommonError::Json(e)))
    }
    pub fn remove_member(&self, group: uuid::Uuid, user: uuid::Uuid) -> Result<(), ClientError> {
        self.client
            .delete(&self.path(&format!("/groups/{group}/members/{user}")))
    }
}
pub fn create_account(
    client: &LogshClient,
    model: &AccountModel,
) -> Result<AccountModel, ClientError> {
    client.post_json("accounts", model)
}

/// Compute replacement permissions while preserving unrelated entries.
pub fn change_permissions(current: Vec<String>, changes: &[String], grant: bool) -> Vec<String> {
    let mut permissions: std::collections::BTreeSet<_> = current.into_iter().collect();
    for permission in changes {
        if grant {
            permissions.insert(permission.clone());
        } else {
            permissions.remove(permission);
        }
    }
    permissions.into_iter().collect()
}
