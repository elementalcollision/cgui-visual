use super::*;

pub async fn list_volumes() -> Result<Vec<Volume>> {
    // Fetch the volume list and the container list in parallel — we need the
    // latter to count how many running containers reference each volume.
    let (vol_res, ctr_res) = tokio::join!(
        run(&["volume", "ls", "--format", "json"]),
        run(&["ls", "--all", "--format", "json"]),
    );
    let bytes = vol_res?;
    if bytes.iter().all(|b| b.is_ascii_whitespace()) {
        return Ok(vec![]);
    }
    let raw: Vec<Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    let refs = ctr_res.map(|b| volume_ref_counts(&b)).unwrap_or_default();

    Ok(raw.into_iter().map(|v| parse_volume(v, &refs)).collect())
}

pub async fn list_networks() -> Result<Vec<Network>> {
    // Mirrors `list_volumes` above: fetch the network list and the
    // container list in parallel, then derive per-network attached-
    // container counts from the latter.
    let (net_res, ctr_res) = tokio::join!(
        run(&["network", "ls", "--format", "json"]),
        run(&["ls", "--all", "--format", "json"]),
    );
    let bytes = net_res?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    // A failed `ls` just means we can't compute counts — fall back to 0
    // rather than failing the whole network list over it.
    let refs = ctr_res.map(|b| network_ref_counts(&b)).unwrap_or_default();
    Ok(raw.into_iter().map(|n| parse_network(n, &refs)).collect())
}

pub async fn inspect_volume(name: &str) -> Result<String> {
    reject_flag_like("volume name", name)?;
    inspect_args(&["volume", "inspect", name]).await
}

pub async fn inspect_network(id: &str) -> Result<String> {
    reject_flag_like("network name", id)?;
    inspect_args(&["network", "inspect", id]).await
}

// ─── Create volume / network ──────────────────────────────────────────
// `container volume create [-s SIZE] [--label k=v]... <name>` and
// `container network create [--subnet CIDR] [--subnet-v6 PREFIX]
// [--internal] [--label k=v]... <name>`. Values are validated here so the
// user gets a specific reason instead of the CLI's terse error.
/// Volume name: `^[A-Za-z0-9][A-Za-z0-9_.-]*$`, up to 255 characters (the
/// CLI accepts far longer names than containers, which cap at 63).
pub fn validate_volume_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 255
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if ok {
        Ok(())
    } else {
        Err(anyhow!(
            "volume name `{name}` must be 1-255 letters, digits, `_`, `.` or `-`, starting with a letter or digit"
        ))
    }
}

/// Network name: `^[a-z0-9](?:[a-z0-9._-]{0,61}[a-z0-9])?$` (lowercase,
/// up to 63 characters, alphanumeric at both ends).
pub fn validate_network_name(name: &str) -> Result<()> {
    let b = name.as_bytes();
    let inner =
        |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-');
    let edge = |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    let ok = !b.is_empty()
        && b.len() <= 63
        && b.first().is_some_and(edge)
        && b.last().is_some_and(edge)
        && b.iter().all(inner);
    if ok {
        Ok(())
    } else {
        Err(anyhow!(
            "network name `{name}` must be 1-63 lowercase letters, digits, `.`, `_` or `-`, starting and ending with a letter or digit"
        ))
    }
}

/// Volume size: a number with an optional K/M/G/T/P suffix (and optional
/// trailing `B`), e.g. `512M`, `10G`, `1.5G`. The CLI enforces the minimum.
pub fn validate_volume_size(size: &str) -> Result<()> {
    let s = size.trim_end_matches(['b', 'B']);
    let s = s
        .strip_suffix(['k', 'K', 'm', 'M', 'g', 'G', 't', 'T', 'p', 'P'])
        .unwrap_or(s);
    let (int, frac) = s.split_once('.').unwrap_or((s, "0"));
    let digits = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit());
    if digits(int) && digits(frac) && !size.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "volume size `{size}` must be a number with an optional K, M, G, T or P suffix, e.g. 512M or 10G"
        ))
    }
}

/// `key=value` label with a non-empty key and no whitespace in the key.
pub fn validate_label(label: &str) -> Result<()> {
    match label.split_once('=') {
        Some((k, _)) if !k.is_empty() && !k.chars().any(char::is_whitespace) => Ok(()),
        _ => Err(anyhow!("label `{label}` must be key=value")),
    }
}

/// IPv4 subnet in CIDR form (`192.168.100.0/24`).
pub fn validate_subnet_v4(cidr: &str) -> Result<()> {
    let ok = cidr.split_once('/').is_some_and(|(ip, len)| {
        ip.parse::<std::net::Ipv4Addr>().is_ok() && len.parse::<u8>().is_ok_and(|l| l <= 32)
    });
    if ok {
        Ok(())
    } else {
        Err(anyhow!(
            "subnet `{cidr}` must be an IPv4 CIDR such as 192.168.100.0/24"
        ))
    }
}

/// IPv6 prefix in CIDR form (`fd00:1234::/64`).
pub fn validate_subnet_v6(cidr: &str) -> Result<()> {
    let ok = cidr.split_once('/').is_some_and(|(ip, len)| {
        ip.parse::<std::net::Ipv6Addr>().is_ok() && len.parse::<u8>().is_ok_and(|l| l <= 128)
    });
    if ok {
        Ok(())
    } else {
        Err(anyhow!(
            "IPv6 prefix `{cidr}` must be a CIDR such as fd00:1234::/64"
        ))
    }
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCreateArgs {
    pub name: String,
    /// `-s`: e.g. `10G`. Empty/None = the CLI default (a large sparse disk).
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

pub fn volume_create_argv(args: &VolumeCreateArgs) -> Result<Vec<String>> {
    validate_volume_name(&args.name)?;
    let mut argv: Vec<String> = vec!["volume".into(), "create".into()];
    if let Some(size) = args
        .size
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        validate_volume_size(size)?;
        argv.push("-s".into());
        argv.push(size.into());
    }
    for l in args
        .labels
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
    {
        validate_label(l)?;
        argv.push("--label".into());
        argv.push(l.into());
    }
    argv.push(args.name.clone());
    Ok(argv)
}

pub async fn create_volume(args: VolumeCreateArgs) -> Result<()> {
    let argv = volume_create_argv(&args)?;
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    run_with_timeout(&argv_ref, LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkCreateArgs {
    pub name: String,
    /// `--subnet`, IPv4 CIDR.
    #[serde(default)]
    pub subnet: Option<String>,
    /// `--subnet-v6`, IPv6 prefix.
    #[serde(default)]
    pub subnet_v6: Option<String>,
    /// `--internal`: restrict to a host-only network.
    #[serde(default)]
    pub internal: bool,
    #[serde(default)]
    pub labels: Vec<String>,
}

pub fn network_create_argv(args: &NetworkCreateArgs) -> Result<Vec<String>> {
    validate_network_name(&args.name)?;
    let mut argv: Vec<String> = vec!["network".into(), "create".into()];
    let some = |o: &Option<String>| {
        o.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    if let Some(s) = some(&args.subnet) {
        validate_subnet_v4(&s)?;
        argv.push("--subnet".into());
        argv.push(s);
    }
    if let Some(s) = some(&args.subnet_v6) {
        validate_subnet_v6(&s)?;
        argv.push("--subnet-v6".into());
        argv.push(s);
    }
    if args.internal {
        argv.push("--internal".into());
    }
    for l in args
        .labels
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
    {
        validate_label(l)?;
        argv.push("--label".into());
        argv.push(l.into());
    }
    argv.push(args.name.clone());
    Ok(argv)
}

pub async fn create_network(args: NetworkCreateArgs) -> Result<()> {
    let argv = network_create_argv(&args)?;
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    run_with_timeout(&argv_ref, LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn delete_volume(name: &str) -> Result<()> {
    reject_flag_like("volume name", name)?;
    run(&["volume", "delete", name]).await.map(|_| ())
}

pub async fn delete_network(id: &str) -> Result<()> {
    reject_flag_like("network name", id)?;
    run(&["network", "delete", id]).await.map(|_| ())
}
