// End-to-end smoke test of the runtime layer against the live `container` CLI.
// Runs two polls 2.5s apart so the second one has real CPU% deltas.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("== smoke: runtime::available()");
    let avail = runtime_available().await;
    println!("  available = {avail}");
    if !avail {
        return Ok(());
    }

    let history = std::sync::Arc::new(state_history_new());
    println!("\n== smoke: poll #1 (baseline)");
    let cs1 = state_poll_once(&history).await?;
    print_containers(&cs1);

    println!("\n  sleeping 2.5s for delta window…");
    tokio::time::sleep(std::time::Duration::from_millis(2500)).await;

    println!("\n== smoke: poll #2 (with deltas)");
    let cs2 = state_poll_once(&history).await?;
    print_containers(&cs2);

    println!("\n== smoke: list_images");
    let imgs = runtime_list_images().await?;
    for i in imgs.iter().take(3) {
        println!(
            "  {} {} {} layers={}",
            i.id, i.reference, i.size_unit, i.layers
        );
    }

    println!("\n== smoke: list_volumes");
    let vols = runtime_list_volumes().await?;
    for v in vols.iter().take(3) {
        println!("  {} {:.2} / {:.2} {}", v.name, v.used, v.size, v.unit);
    }

    println!("\n== smoke: list_networks");
    let nets = runtime_list_networks().await?;
    for n in nets.iter().take(3) {
        println!(
            "  {} mode={} state={} subnet={}",
            n.name, n.mode, n.state, n.subnet
        );
    }

    println!("\n== smoke: stacks::list_stacks");
    let stacks = app_lib::stacks::list_stacks().await;
    if stacks.is_empty() {
        println!("  (no stacks at ~/.config/cgui/stacks/)");
    } else {
        for s in &stacks {
            println!(
                "  {} services={} health={}",
                s.name,
                s.services.len(),
                s.health
            );
            for sv in &s.services {
                println!("    - {} state={} image={}", sv.name, sv.state, sv.image);
            }
        }
    }

    println!("\n== smoke: updates::check (live GitHub API)");
    let updates = app_lib::updates::check().await;
    if updates.is_empty() {
        println!("  (all components up to date or unreachable)");
    } else {
        for u in &updates {
            println!("  {} {} → {}", u.component, u.installed, u.latest);
        }
    }

    Ok(())
}

fn print_containers(cs: &[app_lib::model::Container]) {
    for c in cs {
        println!(
            "  {} {} status={} uptime={} cpu={:.1}% mem={:.2}/{:.0} GiB net={:.0} B/s disk={:.0} B/s cmd={:?}",
            c.id, c.name, c.status, c.uptime, c.cpu, c.mem.used, c.mem.limit,
            c.net_io_bps, c.disk_io_bps, c.cmd,
        );
    }
}

// app_lib doesn't currently re-export these — the binary uses the public
// surface that the smoke test needs.
async fn runtime_available() -> bool {
    app_lib::runtime::available().await
}
async fn runtime_list_images() -> anyhow::Result<Vec<app_lib::model::Image>> {
    app_lib::runtime::list_images().await
}
async fn runtime_list_volumes() -> anyhow::Result<Vec<app_lib::model::Volume>> {
    app_lib::runtime::list_volumes().await
}
async fn runtime_list_networks() -> anyhow::Result<Vec<app_lib::model::Network>> {
    app_lib::runtime::list_networks().await
}
fn state_history_new() -> app_lib::state::History {
    app_lib::state::History::new()
}
async fn state_poll_once(
    h: &app_lib::state::History,
) -> anyhow::Result<Vec<app_lib::model::Container>> {
    app_lib::state::poll_once(h).await
}
