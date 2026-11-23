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

// ─── create volume / network ──────────────────────────────────────────

#[test]
fn volume_names() {
    for n in ["data", "a", "My_Vol-1.x", &"x".repeat(255)] {
        assert!(validate_volume_name(n).is_ok(), "{n}");
    }
    for n in [
        "",
        "-data",
        "_data",
        ".x",
        "has space",
        "a/b",
        &"x".repeat(256),
    ] {
        assert!(validate_volume_name(n).is_err(), "{n:?}");
    }
}

#[test]
fn network_names() {
    for n in ["net", "a", "my-net_1.x", "9lives", &"a".repeat(63)] {
        assert!(validate_network_name(n).is_ok(), "{n}");
    }
    for n in [
        "",
        "Upper",
        "-net",
        "net-",
        "_net",
        "net.",
        "has space",
        &"a".repeat(64),
    ] {
        assert!(validate_network_name(n).is_err(), "{n:?}");
    }
}

#[test]
fn volume_sizes() {
    for s in ["1G", "512m", "10GB", "1048576", "1.5G", "64M", "2T"] {
        assert!(validate_volume_size(s).is_ok(), "{s}");
    }
    for s in ["", "G", "abc", "-1G", "1..5G", "1G2", "10 G", "1.G", ".5G"] {
        assert!(validate_volume_size(s).is_err(), "{s:?}");
    }
}

#[test]
fn subnets_and_labels() {
    assert!(validate_subnet_v4("192.168.100.0/24").is_ok());
    for bad in [
        "192.168.100.0",
        "192.168.100.0/33",
        "300.1.1.0/24",
        "nope",
        "10.0.0.0/",
    ] {
        assert!(validate_subnet_v4(bad).is_err(), "{bad}");
    }
    assert!(validate_subnet_v6("fd00:1234::/64").is_ok());
    for bad in ["fd00::", "fd00::/129", "192.168.1.0/24", "zz::/64"] {
        assert!(validate_subnet_v6(bad).is_err(), "{bad}");
    }
    assert!(validate_label("team=infra").is_ok());
    assert!(validate_label("empty=").is_ok());
    for bad in ["novalue", "=v", "a b=c", ""] {
        assert!(validate_label(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn volume_create_argv_builds_flags_and_validates() {
    let a = VolumeCreateArgs {
        name: "data".into(),
        size: Some(" 10G ".into()),
        labels: vec!["team=infra".into(), "".into(), "env=dev".into()],
    };
    assert_eq!(
        volume_create_argv(&a).unwrap(),
        [
            "volume",
            "create",
            "-s",
            "10G",
            "--label",
            "team=infra",
            "--label",
            "env=dev",
            "data"
        ]
    );
    let bare = VolumeCreateArgs {
        name: "data".into(),
        ..Default::default()
    };
    assert_eq!(
        volume_create_argv(&bare).unwrap(),
        ["volume", "create", "data"]
    );
    let blank_size = VolumeCreateArgs {
        name: "data".into(),
        size: Some("  ".into()),
        ..Default::default()
    };
    assert_eq!(volume_create_argv(&blank_size).unwrap().len(), 3);
    for bad in [
        VolumeCreateArgs {
            name: "--all".into(),
            ..Default::default()
        },
        VolumeCreateArgs {
            name: "data".into(),
            size: Some("big".into()),
            ..Default::default()
        },
        VolumeCreateArgs {
            name: "data".into(),
            labels: vec!["nokey".into()],
            ..Default::default()
        },
    ] {
        assert!(volume_create_argv(&bad).is_err());
    }
}

#[test]
fn network_create_argv_builds_flags_and_validates() {
    let a = NetworkCreateArgs {
        name: "backend".into(),
        subnet: Some("192.168.99.0/24".into()),
        subnet_v6: Some("fd00:99::/64".into()),
        internal: true,
        labels: vec!["team=infra".into()],
    };
    assert_eq!(
        network_create_argv(&a).unwrap(),
        [
            "network",
            "create",
            "--subnet",
            "192.168.99.0/24",
            "--subnet-v6",
            "fd00:99::/64",
            "--internal",
            "--label",
            "team=infra",
            "backend"
        ]
    );
    let bare = NetworkCreateArgs {
        name: "backend".into(),
        ..Default::default()
    };
    assert_eq!(
        network_create_argv(&bare).unwrap(),
        ["network", "create", "backend"]
    );
    for bad in [
        NetworkCreateArgs {
            name: "Backend".into(),
            ..Default::default()
        },
        NetworkCreateArgs {
            name: "-x".into(),
            ..Default::default()
        },
        NetworkCreateArgs {
            name: "n".into(),
            subnet: Some("nope".into()),
            ..Default::default()
        },
        NetworkCreateArgs {
            name: "n".into(),
            subnet_v6: Some("10.0.0.0/8".into()),
            ..Default::default()
        },
    ] {
        assert!(network_create_argv(&bad).is_err());
    }
}

// ─── machines ─────────────────────────────────────────────────────────

#[test]
fn machine_user_args_render_in_order() {
    assert!(machine_user_args(&MachineUserOpts::default())
        .unwrap()
        .is_empty());
    let o = MachineUserOpts {
        user: Some(" dev ".into()),
        uid: Some(1234),
        gid: Some(0),
        home: Some("/home/dev".into()),
    };
    assert_eq!(
        machine_user_args(&o).unwrap(),
        [
            "--user",
            "dev",
            "--uid",
            "1234",
            "--gid",
            "0",
            "--home",
            "/home/dev"
        ]
    );
    // `name|uid[:gid]` is passed through as typed.
    let o = MachineUserOpts {
        user: Some("1000:1000".into()),
        ..Default::default()
    };
    assert_eq!(machine_user_args(&o).unwrap(), ["--user", "1000:1000"]);
    // Blank strings mean "unset".
    let o = MachineUserOpts {
        user: Some("  ".into()),
        home: Some("".into()),
        ..Default::default()
    };
    assert!(machine_user_args(&o).unwrap().is_empty());
}

#[test]
fn machine_user_args_reject_bad_values() {
    for (user, home) in [
        (Some("-x"), None),
        (Some("a b"), None),
        (None, Some("home/dev")),
        (None, Some("~/dev")),
    ] {
        let o = MachineUserOpts {
            user: user.map(String::from),
            home: home.map(String::from),
            ..Default::default()
        };
        assert!(machine_user_args(&o).is_err(), "{user:?} {home:?}");
    }
}

#[test]
fn machine_user_opts_deserialize_from_the_ui() {
    let o: MachineUserOpts =
        serde_json::from_str(r#"{"user":"dev","uid":501,"home":"/home/dev"}"#).unwrap();
    assert_eq!(o.uid, Some(501));
    assert_eq!(o.gid, None);
    let none: MachineUserOpts = serde_json::from_str("{}").unwrap();
    assert!(none.user.is_none() && none.uid.is_none() && none.home.is_none());
}

#[tokio::test]
async fn machine_calls_validate_before_spawning() {
    assert!(machine_logs("--boot", false, None).await.is_err());
    assert!(machine_inspect("-x").await.is_err());
    let bad = MachineUserOpts {
        home: Some("relative".into()),
        ..Default::default()
    };
    assert!(machine_create("dev1", "alpine", None, None, true, &bad)
        .await
        .is_err());
    assert!(machine_create(
        "dev1",
        "--evil",
        None,
        None,
        true,
        &MachineUserOpts::default()
    )
    .await
    .is_err());
}

#[test]
fn create_args_deserialize_from_the_ui() {
    let v: VolumeCreateArgs =
        serde_json::from_str(r#"{"name":"d","size":"1G","labels":["a=b"]}"#).unwrap();
    assert_eq!((v.name.as_str(), v.size.as_deref()), ("d", Some("1G")));
    let n: NetworkCreateArgs =
        serde_json::from_str(r#"{"name":"n","subnetV6":"fd00::/64","internal":true}"#).unwrap();
    assert!(n.internal);
    assert_eq!(n.subnet_v6.as_deref(), Some("fd00::/64"));
}
