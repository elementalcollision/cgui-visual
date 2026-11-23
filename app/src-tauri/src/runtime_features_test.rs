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
