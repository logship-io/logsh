use logsh_core::{
    administration::*,
    connect::Connection,
    logship_client::LogshClient,
    session::{self, LoginMethod, LoginRequest},
};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;

mod common;
use common::{request, Expected, Server};
fn id() -> uuid::Uuid {
    uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
}
fn user() -> Value {
    json!({"userId":id(),"userName":"alice","firstName":"Alice","lastName":"Smith","nickName":""})
}

#[test]
fn account_admin_routes_payloads_and_empty_success() {
    let base = format!("/accounts/{}", id());
    let account = json!({"accountId":id(),"accountName":"prod"});
    let mapping =
        json!({"accountId":id(),"accountName":"prod","userId":id(),"permissions":["Query"]});
    let group = json!({"accountId":id(),"groupId":id(),"name":"ops","description":null,"createdByUserId":uuid::Uuid::nil()});
    let member = json!({"accountId":id(),"groupId":id(),"userId":id()});
    let schema = json!([{"schemaName":"Logs","canRead":true,"canWrite":false,"columns":[]}]);
    let mut setting = request(
        "PUT",
        format!("{base}/settings/a%2Fb"),
        Some(json!({"value":true})),
        Value::Null,
    );
    setting.status = 204;
    let mut delete_group = request(
        "DELETE",
        format!("{base}/groups/{}", id()),
        None,
        Value::Null,
    );
    delete_group.status = 204;
    let server = Server::new(vec![
        request("POST", "/accounts", Some(account.clone()), account.clone()),
        request("GET", &base, None, account.clone()),
        request("PUT", &base, Some(account.clone()), account),
        request("GET", format!("{base}/permissions"), None, json!(["Query"])),
        request("GET", format!("{base}/users"), None, json!([user()])),
        request(
            "GET",
            format!("{base}/users/{}", id()),
            None,
            json!({"user":user(),"permissions":["Query"]}),
        ),
        request(
            "POST",
            format!("{base}/users"),
            Some(json!({"userName":"alice","permissions":["Query"]})),
            mapping.clone(),
        ),
        request(
            "POST",
            format!("{base}/users/{}", id()),
            Some(json!({"permissions":["Query"]})),
            mapping,
        ),
        request(
            "DELETE",
            format!("{base}/users/{}", id()),
            None,
            Value::Null,
        ),
        request(
            "GET",
            format!("{base}/users/{}/schema-permissions", id()),
            None,
            schema.clone(),
        ),
        request(
            "PUT",
            format!("{base}/users/{}/schema-permissions", id()),
            Some(schema),
            Value::Null,
        ),
        request("GET", format!("{base}/settings"), None, json!([])),
        setting,
        request(
            "GET",
            format!("{base}/groups"),
            None,
            json!([group.clone()]),
        ),
        request(
            "PUT",
            format!("{base}/groups/{}", id()),
            Some(group.clone()),
            group.clone(),
        ),
        delete_group,
        request(
            "GET",
            format!("{base}/groups/{}/members", id()),
            None,
            json!([member.clone()]),
        ),
        request(
            "PUT",
            format!("{base}/groups/{}/members/{}", id(), id()),
            None,
            member,
        ),
        request(
            "DELETE",
            format!("{base}/groups/{}/members/{}", id(), id()),
            None,
            Value::Null,
        ),
    ]);
    let client = LogshClient::new(&format!("{}/", server.url), "test-token".into());
    let api = AccountApi {
        client: &client,
        account_id: id(),
    };
    create_account(
        &client,
        &AccountModel {
            account_id: id(),
            account_name: "prod".into(),
        },
    )
    .unwrap();
    api.show().unwrap();
    api.update("prod".into()).unwrap();
    api.permissions().unwrap();
    api.users().unwrap();
    api.user(id()).unwrap();
    api.add_user("alice".into(), vec!["Query".into()]).unwrap();
    api.set_permissions(id(), vec!["Query".into()]).unwrap();
    api.remove_user(id()).unwrap();
    let schemas = api.schema_permissions(id()).unwrap();
    api.set_schema_permissions(id(), &schemas).unwrap();
    api.settings().unwrap();
    api.set_setting("a/b", json!(true)).unwrap();
    api.groups().unwrap();
    api.put_group(&serde_json::from_value(group).unwrap())
        .unwrap();
    api.delete_group(id()).unwrap();
    api.members(id()).unwrap();
    api.add_member(id(), id()).unwrap();
    api.remove_member(id(), id()).unwrap();
    server.verify();
}

#[test]
fn login_never_chooses_an_account_and_clears_stale_selection() {
    for count in [0, 1, 2] {
        let accounts:Vec<_>=(0..count).map(|i|json!({"accountId":uuid::Uuid::from_u128(i+1),"accountName":format!("account-{i}"),"permissions":[]})).collect();
        let server = Server::new(vec![
            request(
                "POST",
                "/auth/token",
                Some(json!({"username":"alice","password":"secret"})),
                json!({"token":"test-token"}),
            ),
            request("GET", "/whoami", None, user()),
            request(
                "GET",
                format!("/users/{}/accounts", id()),
                None,
                json!(accounts),
            ),
        ]);
        let mut previous = Connection::new(&server.url);
        previous.default_account = Some(uuid::Uuid::from_u128(99));
        previous.default_account_name = Some("removed".into());
        let conn = session::authenticate(
            LoginRequest {
                server: server.url.clone(),
                method: LoginMethod::Password,
                username: "alice".into(),
                secret: "secret".into(),
            },
            Some(previous),
            &|_| {},
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(conn.default_account, None);
        assert_eq!(conn.default_account_name, None);
        assert_eq!(conn.known_accounts.len(), count as usize);
        server.verify();
    }
}

#[test]
fn login_preserves_an_accessible_selection() {
    let server = Server::new(vec![
        request("POST", "/auth/token", None, json!({"token":"test-token"})),
        request("GET", "/whoami", None, user()),
        request(
            "GET",
            format!("/users/{}/accounts", id()),
            None,
            json!([{"accountId":id(),"accountName":"renamed","permissions":[]}]),
        ),
    ]);
    let mut previous = Connection::new(&server.url);
    previous.default_account = Some(id());
    let conn = session::authenticate(
        LoginRequest {
            server: server.url.clone(),
            method: LoginMethod::Password,
            username: "alice".into(),
            secret: "secret".into(),
        },
        Some(previous),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(conn.default_account, Some(id()));
    assert_eq!(conn.default_account_name.as_deref(), Some("renamed"));
    server.verify();
}

#[test]
fn permission_changes_preserve_unrelated_permissions() {
    assert_eq!(
        change_permissions(
            vec!["Read".into(), "Admin".into()],
            &["Write".into(), "Read".into()],
            true
        ),
        vec!["Admin", "Read", "Write"]
    );
    assert_eq!(
        change_permissions(vec!["Read".into(), "Admin".into()], &["Read".into()], false),
        vec!["Admin"]
    );
}

#[test]
fn http_errors_keep_status_and_plain_body() {
    let server = Server::new(vec![Expected {
        method: "GET",
        path: "/accounts".into(),
        body: None,
        status: 403,
        response: json!({"message":"Denied","errors":[]}),
    }]);
    let client = LogshClient::new(&server.url, "test-token".into());
    assert!(
        matches!(client.get_json::<Value>("accounts"),Err(logsh_core::error::ClientError::Http{status:403,message}) if message=="Denied")
    );
    server.verify();
}

#[test]
fn account_names_must_be_unambiguous_and_ids_work() {
    let accounts = vec![
        logsh_core::account::AccountsModel {
            account_id: id(),
            account_name: "Prod".into(),
            permissions: vec![],
        },
        logsh_core::account::AccountsModel {
            account_id: uuid::Uuid::from_u128(2),
            account_name: "prod".into(),
            permissions: vec![],
        },
    ];
    assert!(logsh_core::account::resolve_account(&accounts, "prod").is_err());
    assert!(logsh_core::account::resolve_account(&accounts, "missing").is_err());
    assert_eq!(
        logsh_core::account::resolve_account(&accounts, &id().to_string())
            .unwrap()
            .account_id,
        id()
    );
}

fn oauth_setup() -> Vec<Expected> {
    vec![
        request(
            "GET",
            "/auth/oauth",
            None,
            json!({"clientId":"cli","authorizeEndpoint":"$SERVER/authorize","deviceEndpoint":"$SERVER/device","tokenEndpoint":"$SERVER/token","scopes":["openid"]}),
        ),
        request(
            "POST",
            "/device",
            None,
            json!({"device_code":"private-code","user_code":"ABCD","verification_uri":"https://example.invalid/login","expires_in":600,"interval":1}),
        ),
    ]
}

#[test]
fn oauth_device_login_can_be_cancelled_before_polling() {
    let server = Server::new(oauth_setup());
    let cancelled = AtomicBool::new(false);
    let result = session::authenticate(
        LoginRequest {
            server: server.url.clone(),
            method: LoginMethod::Sso,
            username: String::new(),
            secret: String::new(),
        },
        None,
        &|event| {
            let logsh_core::auth::AuthEvent::DeviceCode { url, code } = event;
            assert_eq!(url, "https://example.invalid/login");
            assert_eq!(code, "ABCD");
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        },
        &cancelled,
    );
    assert!(result.err().unwrap().to_string().contains("cancelled"));
    server.verify();
}

#[test]
fn oauth_uses_access_token_expiry_and_emits_device_code() {
    let mut expected = oauth_setup();
    expected.extend([
        request(
            "POST",
            "/token",
            None,
            json!({"access_token":"test-token","token_type":"Bearer","expires_in":3600}),
        ),
        request("GET", "/whoami", None, user()),
        request("GET", format!("/users/{}/accounts", id()), None, json!([])),
    ]);
    let server = Server::new(expected);
    let conn = session::authenticate(
        LoginRequest {
            server: server.url.clone(),
            method: LoginMethod::Sso,
            username: String::new(),
            secret: String::new(),
        },
        None,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    let serialized = serde_json::to_value(conn).unwrap();
    let expiry = serialized["auth"]["OAuth"]["expires"].as_str().unwrap();
    let expiry = chrono::DateTime::parse_from_rfc3339(expiry).unwrap();
    assert!((expiry.timestamp() - chrono::Utc::now().timestamp() - 3600).abs() < 5);
    server.verify();
}

#[test]
fn missing_active_context_does_not_pick_another_server() {
    let mut cfg = logsh_core::config::Configuration {
        current_context: "removed".into(),
        contexts: Default::default(),
    };
    cfg.contexts
        .insert("prod".into(), Connection::new("https://example.invalid"));
    assert!(cfg.get_current_context().is_none());
    assert!(cfg.resolve_current_context().is_err());
}
