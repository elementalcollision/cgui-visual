//! Offline tests for the container 1.6/1.7 features. Child module of
//! `runtime` so it can reach private helpers.

use super::*;

// ─── commit ───────────────────────────────────────────────────────────

#[test]
fn image_ref_accepts_ordinary_references() {
    for r in [
        "myapp",
        "myapp:latest",
        "my-app_1.0:v1.2.3-rc1",
        "registry.example.com/team/app:1",
        "localhost:5000/app",
        "localhost:5000/app:tag",
        "docker.io/library/alpine@sha256:abcdef",
    ] {
        assert!(validate_image_ref("image reference", r).is_ok(), "{r}");
    }
}

#[test]
fn image_ref_rejects_common_mistakes() {
    for (r, why) in [
        ("", "empty"),
        ("-rf", "flag-like"),
        ("my app:latest", "space"),
        ("MyApp:latest", "uppercase repo"),
        ("app:", "empty tag"),
        ("app:bad tag", "space in tag"),
        ("/app", "leading slash"),
        ("host/", "trailing slash"),
        ("app\\x", "backslash"),
    ] {
        assert!(
            validate_image_ref("image reference", r).is_err(),
            "{r:?} should be rejected ({why})"
        );
    }
    let long = format!("{}:latest", "a".repeat(260));
    assert!(validate_image_ref("image reference", &long).is_err());
}

#[test]
fn commit_unsupported_is_translated_only_for_unknown_command() {
    let e =
        anyhow!("`container commit a b` exited exit status: 64: Error: unknown command 'commit'");
    assert_eq!(
        map_commit_unsupported(e).to_string(),
        "`container commit` requires Apple container 1.6 or later"
    );
    let e = anyhow!("`container commit a b` exited exit status: 1: container a not found");
    assert!(map_commit_unsupported(e).to_string().contains("not found"));
}

#[tokio::test]
async fn commit_validates_before_spawning() {
    assert!(commit_container("--all", "app:1").await.is_err());
    assert!(commit_container("web", "--rm").await.is_err());
    assert!(commit_container("web", "Bad Ref").await.is_err());
}

// ─── DNS aliases ──────────────────────────────────────────────────────

#[test]
fn alias_accepts_dns_names() {
    for a in [
        "db",
        "api.example.test",
        "a-b.c-d.example",
        "DB1.Example.Test",
        "trailing.dot.",
    ] {
        assert!(validate_alias(a).is_ok(), "{a}");
    }
    assert!(validate_alias(&format!("{}.test", "a".repeat(63))).is_ok());
}

#[test]
fn alias_rejects_non_dns_names_and_option_injection() {
    for a in [
        "",
        " ",
        "has space.test",
        "a,alias=b.test",
        "a=b.test",
        "-lead.test",
        "trail-.test",
        "under_score.test",
        "double..dot",
        ".leading.dot",
        "emoji\u{1F600}.test",
    ] {
        assert!(validate_alias(a).is_err(), "{a:?}");
    }
    assert!(validate_alias(&format!("{}.test", "a".repeat(64))).is_err());
    assert!(validate_alias(&format!("{}.test", "a.".repeat(130))).is_err());
}

#[test]
fn network_spec_builds_alias_options() {
    let none: [String; 0] = [];
    assert_eq!(network_spec(None, &none), None);
    assert_eq!(network_spec(Some(""), &none), None);
    assert_eq!(
        network_spec(Some("backend"), &none).as_deref(),
        Some("backend")
    );
    let al = vec!["db.example.test".to_string(), "pg.example.test".to_string()];
    assert_eq!(
        network_spec(Some("backend"), &al).as_deref(),
        Some("backend,alias=db.example.test,alias=pg.example.test")
    );
    // Aliases without a network use the default network.
    assert_eq!(
        network_spec(None, &al[..1]).as_deref(),
        Some("default,alias=db.example.test")
    );
    // Blank entries are skipped.
    assert_eq!(
        network_spec(Some("n"), &["".to_string()]).as_deref(),
        Some("n")
    );
}

#[test]
fn run_image_argv_emits_aliases_on_the_network() {
    let args = RunArgs {
        image: "alpine".into(),
        network: Some("backend".into()),
        aliases: vec!["db.example.test".into(), "pg.example.test".into()],
        ..Default::default()
    };
    let argv = run_image_argv(&args).unwrap();
    let i = argv.iter().position(|a| a == "--network").unwrap();
    assert_eq!(
        argv[i + 1],
        "backend,alias=db.example.test,alias=pg.example.test"
    );
    // Exactly one --network flag, and the image still comes after it.
    assert_eq!(argv.iter().filter(|a| *a == "--network").count(), 1);
    assert!(argv.iter().position(|a| a == "alpine").unwrap() > i);

    let bare = RunArgs {
        image: "alpine".into(),
        aliases: vec!["x.example.test".into()],
        ..Default::default()
    };
    let argv = run_image_argv(&bare).unwrap();
    let i = argv.iter().position(|a| a == "--network").unwrap();
    assert_eq!(argv[i + 1], "default,alias=x.example.test");
}

#[test]
fn run_image_argv_rejects_bad_aliases_and_omits_empty_ones() {
    let bad = RunArgs {
        image: "alpine".into(),
        network: Some("n".into()),
        aliases: vec!["ok.test".into(), "no good".into()],
        ..Default::default()
    };
    assert!(run_image_argv(&bad).is_err());
    let blank = RunArgs {
        image: "alpine".into(),
        aliases: vec!["".into()],
        ..Default::default()
    };
    assert!(!run_image_argv(&blank)
        .unwrap()
        .iter()
        .any(|a| a == "--network"));
}

#[test]
fn run_args_deserialize_aliases_from_the_ui() {
    let args: RunArgs =
        serde_json::from_str(r#"{"image":"alpine","network":"n","aliases":["a.test","b.test"]}"#)
            .unwrap();
    assert_eq!(args.aliases, vec!["a.test", "b.test"]);
    let none: RunArgs = serde_json::from_str(r#"{"image":"alpine"}"#).unwrap();
    assert!(none.aliases.is_empty());
}
