use crate::{account::AccountCommand, OutputMode};
use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};
use logsh_core::administration::{
    self as api, AccountApi, AccountModel, AccountUserSchemaPermissions, GroupApiModel,
};
use serde_json::{json, Value};
use std::io::Read;
use uuid::Uuid;

#[derive(Args)]
pub struct PermissionArgs {
    #[arg(
        long = "permission",
        required_unless_present = "clear",
        conflicts_with = "clear",
        help = "Permission to include; repeat for multiple permissions"
    )]
    pub permissions: Vec<String>,
    #[arg(long, help = "Explicitly replace with no permissions")]
    pub clear: bool,
}

#[derive(Subcommand)]
pub enum UsersCommand {
    /// List the users of this account.
    #[command(visible_alias = "ls")]
    List,
    /// Show an account user's details and permissions.
    Show { user: Uuid },
    /// Add by username; may create and email an invitation to a new user.
    Add {
        username: String,
        #[command(flatten)]
        permissions: PermissionArgs,
    },
    /// Remove a user's access to this account.
    Remove { user: Uuid },
    /// Replace, grant, or revoke account permissions.
    #[command(subcommand)]
    Permissions(PermissionsCommand),
    /// Inspect or replace schema permissions.
    #[command(subcommand)]
    SchemaPermissions(SchemaPermissionsCommand),
}

#[derive(Subcommand)]
pub enum PermissionsCommand {
    /// Replace the entire permission list; use --clear for an empty list.
    Set {
        user: Uuid,
        #[command(flatten)]
        permissions: PermissionArgs,
    },
    /// Read current permissions and add permissions (not an atomic operation).
    Grant {
        user: Uuid,
        #[arg(long = "permission", required = true)]
        permissions: Vec<String>,
    },
    /// Read current permissions and remove permissions (not an atomic operation).
    Revoke {
        user: Uuid,
        #[arg(long = "permission", required = true)]
        permissions: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum SchemaPermissionsCommand {
    /// List schema and column permissions for a user.
    List { user: Uuid },
    /// Replace schema permissions using a JSON array; use --clear for [].
    Set {
        user: Uuid,
        #[arg(
            long,
            required_unless_present = "clear",
            conflicts_with = "clear",
            help = "JSON file; '-' reads stdin"
        )]
        file: Option<String>,
        #[arg(long)]
        clear: bool,
    },
}

#[derive(Subcommand)]
pub enum SettingsCommand {
    /// List account settings and their descriptions.
    #[command(visible_alias = "ls")]
    List,
    /// Set a named setting; JSON strings must include quotes.
    Set {
        name: String,
        #[arg(long)]
        value_json: String,
    },
}

#[derive(Subcommand)]
pub enum GroupsCommand {
    /// List account groups.
    #[command(visible_alias = "ls")]
    List,
    /// Create a group; generates an ID when omitted.
    Create {
        name: String,
        #[arg(long)]
        id: Option<Uuid>,
        #[arg(long)]
        description: Option<String>,
    },
    /// Update a group, preserving fields not supplied.
    Update {
        group: Uuid,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete an account group.
    Delete { group: Uuid },
    /// List, add, and remove group members.
    #[command(subcommand)]
    Members(MembersCommand),
}

#[derive(Subcommand)]
pub enum MembersCommand {
    List { group: Uuid },
    Add { group: Uuid, user: Uuid },
    Remove { group: Uuid, user: Uuid },
}

fn value<T: serde::Serialize>(value: T) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

pub fn execute(command: AccountCommand, output: Option<OutputMode>) -> Result<()> {
    let connection = logsh_core::config::load()?
        .resolve_current_context()?
        .connection;
    let token = connection
        .get_token()
        .ok_or_else(|| anyhow!("Not authenticated. Run logsh context login."))?;
    let client = logsh_core::logship_client::LogshClient::new(&connection.server, token);
    if let AccountCommand::Create { name, id } = command {
        nonempty(&name, "Account name")?;
        return print(
            &value(api::create_account(
                &client,
                &AccountModel {
                    account_id: id.unwrap_or_else(Uuid::new_v4),
                    account_name: name,
                },
            )?)?,
            output,
        );
    }
    let account_id = connection.effective_account()?.ok_or_else(|| {
        anyhow!("No account selected. Run logsh account list, then logsh account use <name-or-id>.")
    })?;
    let api = AccountApi {
        client: &client,
        account_id,
    };
    let result = match command {
        AccountCommand::Show => value(api.show()?)?,
        AccountCommand::Update { name } => {
            nonempty(&name, "Account name")?;
            value(api.update(name)?)?
        }
        AccountCommand::Permissions => value(api.permissions()?)?,
        AccountCommand::Users(command) => match command {
            UsersCommand::List => value(api.users()?)?,
            UsersCommand::Show { user } => value(api.user(user)?)?,
            UsersCommand::Add {
                username,
                permissions,
            } => {
                nonempty(&username, "Username")?;
                validate_permissions(&permissions.permissions)?;
                value(api.add_user(username, permissions.permissions)?)?
            }
            UsersCommand::Remove { user } => {
                api.remove_user(user)?;
                json!({"status":"ok", "userId":user})
            }
            UsersCommand::Permissions(command) => {
                let (user, permissions) = match command {
                    PermissionsCommand::Set { user, permissions } => {
                        validate_permissions(&permissions.permissions)?;
                        (user, permissions.permissions)
                    }
                    PermissionsCommand::Grant { user, permissions } => {
                        validate_permissions(&permissions)?;
                        (
                            user,
                            api::change_permissions(
                                api.user(user)?.permissions,
                                &permissions,
                                true,
                            ),
                        )
                    }
                    PermissionsCommand::Revoke { user, permissions } => {
                        validate_permissions(&permissions)?;
                        (
                            user,
                            api::change_permissions(
                                api.user(user)?.permissions,
                                &permissions,
                                false,
                            ),
                        )
                    }
                };
                value(api.set_permissions(user, permissions)?)?
            }
            UsersCommand::SchemaPermissions(command) => match command {
                SchemaPermissionsCommand::List { user } => value(api.schema_permissions(user)?)?,
                SchemaPermissionsCommand::Set { user, file, clear } => {
                    let permissions: Vec<AccountUserSchemaPermissions> = if clear {
                        vec![]
                    } else {
                        let file = file.ok_or_else(|| anyhow!("Supply --file or --clear"))?;
                        let text = if file == "-" {
                            let mut text = String::new();
                            std::io::stdin().read_to_string(&mut text)?;
                            text
                        } else {
                            std::fs::read_to_string(file)?
                        };
                        let permissions: Vec<AccountUserSchemaPermissions> =
                            serde_json::from_str(&text)?;
                        if permissions.is_empty() {
                            return Err(anyhow!(
                                "Use --clear to replace with no schema permissions."
                            ));
                        }
                        permissions
                    };
                    api.set_schema_permissions(user, &permissions)?;
                    json!({"status":"ok", "userId":user})
                }
            },
        },
        AccountCommand::Settings(command) => match command {
            SettingsCommand::List => value(api.settings()?)?,
            SettingsCommand::Set { name, value_json } => {
                nonempty(&name, "Setting name")?;
                api.set_setting(&name, serde_json::from_str(&value_json)?)?;
                json!({"status":"ok", "setting":name})
            }
        },
        AccountCommand::Groups(command) => match command {
            GroupsCommand::List => value(api.groups()?)?,
            GroupsCommand::Create {
                name,
                id,
                description,
            } => {
                nonempty(&name, "Group name")?;
                value(api.put_group(&GroupApiModel {
                    account_id,
                    group_id: id.unwrap_or_else(Uuid::new_v4),
                    name,
                    description,
                    created_by_user_id: Uuid::nil(),
                    created_at: String::new(),
                    updated_at: String::new(),
                })?)?
            }
            GroupsCommand::Update {
                group,
                name,
                description,
            } => {
                if name.is_none() && description.is_none() {
                    return Err(anyhow!("Supply --name or --description."));
                }
                let mut existing = api
                    .groups()?
                    .into_iter()
                    .find(|g| g.group_id == group)
                    .ok_or_else(|| anyhow!("Group not found"))?;
                if let Some(name) = name {
                    nonempty(&name, "Group name")?;
                    existing.name = name;
                }
                if description.is_some() {
                    existing.description = description;
                }
                value(api.put_group(&existing)?)?
            }
            GroupsCommand::Delete { group } => {
                api.delete_group(group)?;
                json!({"status":"ok", "groupId":group})
            }
            GroupsCommand::Members(command) => match command {
                MembersCommand::List { group } => value(api.members(group)?)?,
                MembersCommand::Add { group, user } => value(api.add_member(group, user)?)?,
                MembersCommand::Remove { group, user } => {
                    api.remove_member(group, user)?;
                    json!({"status":"ok", "groupId":group, "userId":user})
                }
            },
        },
        _ => unreachable!("Existing account commands are handled by account::execute_account"),
    };
    print(&result, output)
}

fn nonempty(value: &str, name: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(anyhow!("{name} must not be empty"))
    } else {
        Ok(())
    }
}
fn validate_permissions(permissions: &[String]) -> Result<()> {
    for p in permissions {
        nonempty(p, "Permission")?;
    }
    Ok(())
}

fn cell(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}
fn flatten(value: &Value, prefix: &str, out: &mut std::collections::BTreeMap<String, String>) {
    if let Value::Object(object) = value {
        for (key, value) in object {
            let key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            flatten(value, &key, out);
        }
    } else {
        out.insert(
            if prefix.is_empty() {
                "value".into()
            } else {
                prefix.into()
            },
            cell(value),
        );
    }
}

pub fn print(value: &Value, output: Option<OutputMode>) -> Result<()> {
    match output.unwrap_or_default() {
        OutputMode::Json => {
            crate::fmt::print_json(value);
            return Ok(());
        }
        OutputMode::JsonPretty => {
            crate::fmt::print_json_pretty(value);
            return Ok(());
        }
        _ => {}
    }
    let items = match value {
        Value::Array(items) => items.clone(),
        _ => vec![value.clone()],
    };
    let rows: Vec<_> = items
        .iter()
        .map(|v| {
            let mut row = std::collections::BTreeMap::new();
            flatten(v, "", &mut row);
            row
        })
        .collect();
    let headers: std::collections::BTreeSet<_> =
        rows.iter().flat_map(|r| r.keys().cloned()).collect();
    if matches!(output, Some(OutputMode::Csv)) {
        let mut writer = csv::Writer::from_writer(std::io::stdout());
        writer.write_record(&headers)?;
        for row in &rows {
            writer.write_record(
                headers
                    .iter()
                    .map(|h| row.get(h).map(String::as_str).unwrap_or("")),
            )?;
        }
        writer.flush()?;
    } else {
        if rows.is_empty() {
            println!("No items found.");
            return Ok(());
        }
        let mut table = term_table::Table::new();
        table.style = if matches!(output, Some(OutputMode::Markdown)) {
            crate::query::markdown_style()
        } else {
            term_table::TableStyle::thin()
        };
        table.add_row(term_table::row::Row::new(
            headers
                .iter()
                .map(term_table::table_cell::TableCell::new)
                .collect::<Vec<_>>(),
        ));
        for row in &rows {
            table.add_row(term_table::row::Row::new(
                headers
                    .iter()
                    .map(|h| {
                        term_table::table_cell::TableCell::new(
                            row.get(h).map(String::as_str).unwrap_or(""),
                        )
                    })
                    .collect::<Vec<_>>(),
            ));
        }
        print!("{}", table.render());
    }
    Ok(())
}
