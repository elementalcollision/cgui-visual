//! Live regression test: container actions must work with the id the app
//! itself parses from `container ls`, including ids longer than 8 chars
//! (the CLI matches ids exactly; no prefix matching).
//!
//! Ignored by default and a no-op unless `CGUI_LIVE=1`. Run with:
//!   CGUI_LIVE=1 cargo test --test live_container_ids -- --ignored --nocapture
//! Requires Apple `container` with services running and alpine:latest
//! pullable. Only touches the container named `cgui-live-ids-<pid>`.

use app_lib::runtime;
use std::process::Command;

struct Cleanup(String);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = Command::new(runtime::current_bin())
            .args(["delete", "--force", &self.0])
            .output();
    }
}

#[tokio::test]
#[ignore]
async fn long_container_id_start_stop_delete() {
    if std::env::var("CGUI_LIVE").as_deref() != Ok("1") {
        eprintln!("CGUI_LIVE != 1; skipping live test");
        return;
    }
    let name = format!("cgui-live-ids-{}", std::process::id());
    assert!(name.len() > 8 && name.starts_with("cgui-live-"));
    let _cleanup = Cleanup(name.clone());

    let out = Command::new(runtime::current_bin())
        .args(["run", "-d", "--name", &name, "alpine:latest", "sleep", "60"])
        .output()
        .expect("spawn container run");
    assert!(
        out.status.success(),
        "container run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let (cs, _) = runtime::list_containers_raw().await.expect("list");
    let c = cs
        .iter()
        .find(|c| c.id == name || c.name == name)
        .unwrap_or_else(|| panic!("{name} not listed"));
    assert_eq!(c.id, name, "Container.id must be the full CLI id");
    assert_eq!(c.short_id, name.chars().take(8).collect::<String>());
    let id = c.id.clone();

    runtime::stop(&id).await.expect("stop with full id");
    runtime::start(&id).await.expect("start with full id");
    runtime::stop(&id).await.expect("stop again");
    runtime::delete(&id).await.expect("delete with full id");
    println!("live ids test ok for {id}");
}
