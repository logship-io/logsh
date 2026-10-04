#[path = "../logsh-core/tests/common/mod.rs"]
mod common;
use common::{request, Server};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

struct Config {
    root: PathBuf,
    path: PathBuf,
}
impl Config {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "logsh-cli-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("nested/config.json");
        Self { root, path }
    }
    fn write(&self, server: &str, account: Option<&str>) {
        std::fs::create_dir_all(self.path.parent().unwrap()).unwrap();
        std::fs::write(&self.path,json!({"current_context":"prod","contexts":{"prod":{"server":server,"user_id":USER,"username":"alice","default_account":account,"default_account_name":account.map(|_|"prod"),"known_accounts":["prod"],"auth":{"Jwt":{"token":"test-token","expires":null}}}}}).to_string()).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_logsh"))
            .args(args)
            .env("LOGSH_CONFIG_PATH", &self.path)
            .env("NO_COLOR", "1")
            .env_remove("LOGSH_PAT_TOKEN")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}
impl Drop for Config {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
const USER: &str = "00000000-0000-0000-0000-000000000001";
const ACCOUNT: &str = "00000000-0000-0000-0000-000000000002";
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn help_is_compact_and_full_help_keeps_advanced_flags() {
    let cfg = Config::new();
    for help in ["-h", "--help"] {
        let result = cfg.run(&["context", "add", help]);
        assert!(result.status.success(), "{}", stderr(&result));
        let text = stdout(&result);
        assert!(text.lines().count() < 40, "{text}");
        assert!(
            text.lines().all(|line| line.chars().count() <= 80),
            "{text}"
        );
        assert!(!text.contains("--config-path"));
        assert!(text.contains("--password-stdin"));
        assert!(text.contains("--help-all"));
    }
    let result = cfg.run(&["context", "add", "--help-all"]);
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(stdout(&result).contains("--config-path"));
    assert!(stdout(&result).contains("--token"));
    let result = cfg.run(&["--help"]);
    assert!(!stdout(&result).contains("__complete"));
    assert!(
        stdout(&result).find("Get started").unwrap() < stdout(&result).find("Options:").unwrap()
    );
}

#[test]
fn first_run_redirected_never_prompts_or_writes() {
    let cfg = Config::new();
    let result = cfg.run(&[]);
    assert_eq!(result.status.code(), Some(3));
    assert!(stderr(&result).contains("Welcome to logsh"));
    assert!(!cfg.path.exists());
    let result = cfg.run(&["-o", "json"]);
    assert_eq!(result.status.code(), Some(3));
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["status"], "notConfigured");
    assert!(result.stderr.is_empty());
    let result = cfg.run(&["context", "add", "https://example.invalid"]);
    assert!(!result.status.success());
    assert!(stderr(&result).contains("--username"));
    assert!(!cfg.path.exists());
}

#[test]
fn bare_context_accepts_global_flags_and_shows_overview() {
    let cfg = Config::new();
    cfg.write("https://example.invalid", Some(ACCOUNT));
    for args in [
        &["context"][..],
        &["--context", "prod", "context"][..],
        &["ctx", "--context", "prod"][..],
    ] {
        let result = cfg.run(args);
        assert!(result.status.success(), "{}", stderr(&result));
        let text = stdout(&result);
        assert!(text.contains("https://example.invalid"));
        assert!(text.contains("logsh context login"));
    }
    let result = cfg.run(&["context", "-o", "json"]);
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(serde_json::from_slice::<Value>(&result.stdout)
        .unwrap()
        .is_array());
    let result = cfg.run(&["--context", "missing", "account", "current"]);
    assert_eq!(result.status.code(), Some(3));
    assert!(stderr(&result).contains("missing"));
}

#[test]
fn account_users_list_uses_selected_account_and_json_output() {
    let server = Server::new(vec![request(
        "GET",
        format!("/accounts/{ACCOUNT}/users"),
        None,
        json!([{"userId":USER,"userName":"alice","firstName":"Alice","lastName":"Smith","nickName":""}]),
    )]);
    let cfg = Config::new();
    cfg.write(&server.url, Some(ACCOUNT));
    let result = cfg.run(&["account", "users", "list", "-o", "json"]);
    assert!(result.status.success(), "{}", stderr(&result));
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value[0]["userName"], "alice");
    server.verify();
}

#[test]
fn setup_requires_explicit_account_in_scripts_and_does_not_write_on_failure() {
    let accounts = json!([{"accountId":ACCOUNT,"accountName":"prod","permissions":[]}]);
    let expected = || {
        vec![
            request("POST", "/auth/token", None, json!({"token":"test-token"})),
            request(
                "GET",
                "/whoami",
                None,
                json!({"userId":USER,"userName":"alice"}),
            ),
            request(
                "GET",
                format!("/users/{USER}/accounts"),
                None,
                accounts.clone(),
            ),
            request(
                "GET",
                format!("/users/{USER}/accounts"),
                None,
                accounts.clone(),
            ),
        ]
    };
    let server = Server::new(expected());
    let cfg = Config::new();
    let result = cfg.run(&[
        "context",
        "add",
        &server.url,
        "--username",
        "alice",
        "--password",
        "secret",
    ]);
    assert!(!result.status.success());
    assert!(stderr(&result).contains("--account"));
    assert!(!cfg.path.exists());
    server.verify();
    let server = Server::new(expected());
    let result = cfg.run(&[
        "context",
        "add",
        &server.url,
        "--name",
        "prod",
        "--username",
        "alice",
        "--password",
        "secret",
        "--account",
        ACCOUNT,
        "-o",
        "json-pretty",
    ]);
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(cfg.path.exists());
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&cfg.path).unwrap()).unwrap();
    assert_eq!(value["contexts"]["prod"]["default_account"], ACCOUNT);
    server.verify();
}

#[test]
fn invalid_account_override_never_uses_saved_account() {
    let server = Server::new(vec![request(
        "GET",
        format!("/users/{USER}/accounts"),
        None,
        json!([{"accountId":ACCOUNT,"accountName":"prod","permissions":[]}]),
    )]);
    let cfg = Config::new();
    cfg.write(&server.url, Some(ACCOUNT));
    let before = std::fs::read(&cfg.path).unwrap();
    let result = cfg.run(&["--account", "missing", "account", "users", "list"]);
    assert!(!result.status.success());
    assert!(stderr(&result).contains("not accessible"));
    assert_eq!(std::fs::read(&cfg.path).unwrap(), before);
    server.verify();
}

#[test]
fn empty_permission_replacements_require_clear() {
    let cfg = Config::new();
    let result = cfg.run(&["account", "users", "permissions", "set", USER]);
    assert!(!result.status.success());
    assert!(stderr(&result).contains("--permission"));
    let result = cfg.run(&[
        "account",
        "users",
        "permissions",
        "set",
        USER,
        "--clear",
        "--permission",
        "Read",
    ]);
    assert!(!result.status.success());
}

#[test]
fn relogin_keeps_the_account_and_supports_script_credentials() {
    let accounts = json!([{"accountId":ACCOUNT,"accountName":"renamed","permissions":[]}]);
    let server = Server::new(vec![
        request(
            "POST",
            "/auth/token",
            Some(json!({"username":"alice","password":"secret"})),
            json!({"token":"test-token"}),
        ),
        request(
            "GET",
            "/whoami",
            None,
            json!({"userId":USER,"userName":"alice"}),
        ),
        request(
            "GET",
            format!("/users/{USER}/accounts"),
            None,
            accounts.clone(),
        ),
        request("GET", format!("/users/{USER}/accounts"), None, accounts),
    ]);
    let cfg = Config::new();
    cfg.write(&server.url, Some(ACCOUNT));
    let result = cfg.run(&[
        "context",
        "login",
        "prod",
        "--password",
        "secret",
        "-o",
        "json",
    ]);
    assert!(result.status.success(), "{}", stderr(&result));
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["account"],
        "renamed"
    );
    server.verify();
}

#[test]
fn full_help_does_not_print_environment_token_values() {
    let result = Command::new(env!("CARGO_BIN_EXE_logsh"))
        .args(["context", "add", "--help-all"])
        .env("LOGSH_PAT_TOKEN", "sensitive-token-value")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(!stdout(&result).contains("sensitive-token-value"));
    assert!(stdout(&result).contains("LOGSH_PAT_TOKEN"));
}
