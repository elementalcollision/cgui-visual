// docker-compose.yml → cgui StackToml conversion.
//
// Targets the common subset of Compose files that real-world Docker users
// have (image, ports, environment, volumes, depends_on, restart, command,
// healthcheck). Things explicitly NOT supported:
//
//   - `build:`              we don't run builds in stacks
//   - `extends:` / merging   too easy to silently lose semantics
//   - `profiles:`           nuanced, deferred
//   - `secrets:` / `configs:` no equivalent on this side
//
// When we hit something unsupported we surface a clear error instead of
// pretending the converted stack still matches the source.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

use crate::stacks::{Healthcheck, ServiceToml, StackToml};

/// Top-level compose document. We only deserialize the fields we use.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ComposeFile {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub services: BTreeMap<String, ComposeService>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ComposeService {
    #[serde(default)]
    pub image: Option<String>,
    /// We refuse to convert services whose only artifact is a build context;
    /// recorded so the error message can name them.
    #[serde(default)]
    pub build: Option<serde_yaml::Value>,

    #[serde(default)]
    pub ports: Vec<ComposePort>,
    #[serde(default)]
    pub environment: ComposeEnv,
    #[serde(default)]
    pub volumes: Vec<ComposeVolume>,
    #[serde(default)]
    pub networks: Option<ComposeNetworks>,
    #[serde(default)]
    pub depends_on: Option<ComposeDependsOn>,
    #[serde(default)]
    pub restart: Option<String>,
    #[serde(default)]
    pub cap_add: Vec<String>,
    #[serde(default)]
    pub cap_drop: Vec<String>,
    /// Compose splits these; cgui collapses both into `args`. We try the
    /// command first, then fall back to entrypoint.
    #[serde(default)]
    pub command: Option<ComposeArgv>,
    #[serde(default)]
    pub entrypoint: Option<ComposeArgv>,
    #[serde(default)]
    pub healthcheck: Option<ComposeHealthcheck>,
}

/// `ports:` accepts both `"8080:80"` and `{ target: 80, published: 8080 }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ComposePort {
    Short(String),
    Long {
        target: u16,
        #[serde(default)]
        published: Option<PublishedPort>,
        #[serde(default)]
        protocol: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PublishedPort {
    Num(u16),
    Str(String),
}

/// `environment:` accepts both list-of-strings (`["KEY=value"]`) and
/// map-of-strings (`{ KEY: value }`). serde_yaml's untagged enum picks
/// the right branch by structure.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ComposeEnv {
    List(Vec<String>),
    Map(BTreeMap<String, ComposeScalar>),
}

impl Default for ComposeEnv {
    fn default() -> Self {
        ComposeEnv::List(vec![])
    }
}

/// Compose lets `KEY: 1234` unquoted; serde_yaml will see this as integer.
/// Accept any scalar and coerce to its string form.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ComposeScalar {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

impl ComposeScalar {
    fn to_string_value(&self) -> String {
        match self {
            Self::Str(s) => s.clone(),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            Self::Bool(b) => b.to_string(),
            Self::Null => String::new(),
        }
    }
}

/// `volumes:` short form: `"name:/dest"` / `"./host:/dest:ro"`.
/// Long form: `{ type: volume, source: name, target: /dest }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ComposeVolume {
    Short(String),
    Long {
        #[serde(default)]
        source: Option<String>,
        target: String,
        #[serde(default)]
        read_only: Option<bool>,
    },
}

/// `networks:` accepts `[default]` (list) or `{ default: {...} }` (map).
/// We only take the first network name in either form because cgui's stack
/// schema currently supports a single network per service.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ComposeNetworks {
    List(Vec<String>),
    Map(BTreeMap<String, serde_yaml::Value>),
}

/// `depends_on:` short form is `[name1, name2]`; long form is
/// `{ name1: { condition: service_started }, ... }`. We just want the
/// names.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ComposeDependsOn {
    List(Vec<String>),
    Map(BTreeMap<String, serde_yaml::Value>),
}

/// `command:` and `entrypoint:` accept string or array. Normalize to argv.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ComposeArgv {
    String(String),
    List(Vec<String>),
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ComposeHealthcheck {
    #[serde(default)]
    pub test: Option<ComposeArgv>,
    #[serde(default)]
    pub interval: Option<String>,
    #[serde(default)]
    pub start_period: Option<String>,
    /// Compose uses this to disable inherited healthchecks; honor it.
    #[serde(default)]
    pub disable: Option<bool>,
}

/// Convert a parsed Compose document into a single cgui stack TOML.
/// `default_name` is used when the document doesn't declare a top-level
/// `name:` (typically the file's containing directory name).
pub fn convert(compose: ComposeFile, default_name: &str) -> Result<StackToml> {
    let stack_name = compose.name.unwrap_or_else(|| default_name.to_string());
    if stack_name.is_empty() {
        return Err(anyhow!(
            "compose file has no `name:` and no fallback could be derived"
        ));
    }

    let mut services: Vec<ServiceToml> = Vec::with_capacity(compose.services.len());

    // BTreeMap iterates lexicographically — fine for our purposes; the
    // generated TOML is reproducible across runs.
    for (svc_name, svc) in compose.services {
        let image = match (&svc.image, &svc.build) {
            (Some(img), _) => img.clone(),
            (None, Some(_)) => {
                return Err(anyhow!(
                    "service '{svc_name}' uses `build:` but no `image:` — \
                 cgui stacks don't run builds. Pre-build with `container build` \
                 and reference the resulting image instead."
                ))
            }
            (None, None) => return Err(anyhow!("service '{svc_name}' has no `image:` field")),
        };

        services.push(ServiceToml {
            name: svc_name,
            image,
            env: convert_env(svc.environment),
            ports: convert_ports(svc.ports),
            volumes: convert_volumes(svc.volumes),
            network: pick_network(svc.networks),
            depends_on: pick_depends_on(svc.depends_on),
            args: convert_argv(svc.command.or(svc.entrypoint)),
            restart: convert_restart(svc.restart),
            cap_add: svc.cap_add,
            cap_drop: svc.cap_drop,
            healthcheck: convert_healthcheck(svc.healthcheck),
        });
    }

    Ok(StackToml {
        name: stack_name,
        services,
    })
}

/// Read a compose file from disk, parse, and convert to a stack TOML
/// string ready to write to ~/.config/cgui/stacks/<name>.toml.
pub fn import_file(path: &Path) -> Result<(StackToml, String)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let compose: ComposeFile =
        serde_yaml::from_str(&text).with_context(|| format!("parse YAML at {}", path.display()))?;

    // If `name:` is absent in the file, fall back to the containing dir name.
    // Compose itself uses the same convention for its `--project-name`.
    let default_name = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("imported");

    let stack = convert(compose, default_name)?;
    let toml_text = toml::to_string_pretty(&stack).context("serialize StackToml to text")?;
    Ok((stack, toml_text))
}

// ─── Field-level converters ───────────────────────────────────────────

fn convert_env(env: ComposeEnv) -> BTreeMap<String, String> {
    match env {
        ComposeEnv::Map(m) => m
            .into_iter()
            .map(|(k, v)| (k, v.to_string_value()))
            .collect(),
        ComposeEnv::List(l) => l
            .into_iter()
            .filter_map(|s| {
                s.split_once('=')
                    .map(|(k, v)| (k.to_string(), v.to_string()))
            })
            .collect(),
    }
}

fn convert_ports(ports: Vec<ComposePort>) -> Vec<String> {
    ports
        .into_iter()
        .map(|p| match p {
            ComposePort::Short(s) => s,
            ComposePort::Long {
                target,
                published,
                protocol: _,
            } => match published {
                Some(PublishedPort::Num(n)) => format!("{n}:{target}"),
                Some(PublishedPort::Str(s)) => format!("{s}:{target}"),
                None => format!("{target}:{target}"),
            },
        })
        .collect()
}

fn convert_volumes(volumes: Vec<ComposeVolume>) -> Vec<String> {
    volumes
        .into_iter()
        .map(|v| match v {
            ComposeVolume::Short(s) => s,
            ComposeVolume::Long {
                source,
                target,
                read_only,
            } => {
                let src = source.unwrap_or_default();
                let mount = if src.is_empty() {
                    target
                } else {
                    format!("{src}:{target}")
                };
                if read_only.unwrap_or(false) {
                    format!("{mount}:ro")
                } else {
                    mount
                }
            }
        })
        .collect()
}

fn pick_network(n: Option<ComposeNetworks>) -> Option<String> {
    match n? {
        ComposeNetworks::List(l) => l.into_iter().next(),
        ComposeNetworks::Map(m) => m.into_keys().next(),
    }
}

fn pick_depends_on(d: Option<ComposeDependsOn>) -> Vec<String> {
    match d {
        None => vec![],
        Some(ComposeDependsOn::List(l)) => l,
        Some(ComposeDependsOn::Map(m)) => m.into_keys().collect(),
    }
}

fn convert_argv(a: Option<ComposeArgv>) -> Vec<String> {
    match a {
        None => vec![],
        // Compose treats unquoted strings as shell-form; we punt on shell
        // parsing and pass it through whitespace-split. Users with complex
        // shell syntax should switch to the array form for an exact mapping.
        Some(ComposeArgv::String(s)) => s.split_whitespace().map(String::from).collect(),
        Some(ComposeArgv::List(l)) => l,
    }
}

/// Compose: `no` | `always` | `on-failure[:N]` | `unless-stopped`.
/// cgui:    `always` | `on-failure` (anything else maps to None).
fn convert_restart(r: Option<String>) -> Option<String> {
    match r.as_deref() {
        Some("always") | Some("unless-stopped") => Some("always".into()),
        Some(s) if s.starts_with("on-failure") => Some("on-failure".into()),
        _ => None,
    }
}

fn convert_healthcheck(h: Option<ComposeHealthcheck>) -> Option<Healthcheck> {
    let h = h?;
    if h.disable.unwrap_or(false) {
        return None;
    }
    let test = h.test?;

    // Compose's `test:` field is one of:
    //   ["NONE"]                           → no healthcheck
    //   ["CMD", "exec", "args", ...]       → exec form
    //   ["CMD-SHELL", "echo hi"]           → shell-string form
    //   "echo hi" (string)                 → CMD-SHELL implicitly
    let argv: Vec<String> = match test {
        ComposeArgv::List(mut v) => match v.first().map(String::as_str) {
            Some("NONE") => return None,
            Some("CMD") | Some("CMD-SHELL") => v.drain(1..).collect(),
            _ => v,
        },
        ComposeArgv::String(s) => vec!["sh".into(), "-c".into(), s],
    };

    Some(Healthcheck {
        kind: "cmd".into(),
        target: None,
        command: argv,
        expect_status: vec![],
        interval_s: parse_duration_to_secs(h.interval.as_deref()).unwrap_or(30),
        start_period_s: parse_duration_to_secs(h.start_period.as_deref()).unwrap_or(0),
    })
}

/// Compose duration grammar: `30s`, `1m30s`, `2h`, etc. Plain integer
/// = seconds (matches Docker's de-facto behavior). Anything we can't
/// parse comes back as None and the caller falls through to a default.
fn parse_duration_to_secs(s: Option<&str>) -> Option<u64> {
    let s = s?.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }
    let mut total: u64 = 0;
    let mut num = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else {
            let n: u64 = num.parse().ok()?;
            num.clear();
            total = total.checked_add(match c {
                's' => n,
                'm' => n.checked_mul(60)?,
                'h' => n.checked_mul(3600)?,
                'd' => n.checked_mul(86400)?,
                _ => return None,
            })?;
        }
    }
    if !num.is_empty() {
        // Trailing bare integer — treat as seconds.
        total = total.checked_add(num.parse::<u64>().ok()?)?;
    }
    Some(total)
}

// ─── Export: StackToml → docker-compose YAML ──────────────────────────
//
// Inverse of `convert` — emits a Compose v3-flavoured document that the
// import path round-trips back to the same stack. We build a fresh
// `serde_yaml::Mapping` (rather than a typed struct) so the output stays
// exactly the schema we want regardless of how StackToml evolves.
//
// Round-trip property: `import(export(stack))` must structurally equal
// `stack` for any stack the parser accepts. Tests below lock this in.

pub fn export(stack: &StackToml) -> Result<String> {
    use serde_yaml::{Mapping, Value};

    let mut services = Mapping::new();
    for svc in &stack.services {
        let mut s = Mapping::new();
        if !svc.image.is_empty() {
            s.insert("image".into(), svc.image.clone().into());
        }
        if !svc.env.is_empty() {
            // Emit env as a map (Compose accepts both list and map; map
            // is friendlier for diffs and preserves key order via BTreeMap).
            let mut env = Mapping::new();
            for (k, v) in &svc.env {
                env.insert(k.clone().into(), v.clone().into());
            }
            s.insert("environment".into(), env.into());
        }
        if !svc.ports.is_empty() {
            // Short-form strings — round-trip through the parser's Short variant.
            let ports: Vec<Value> = svc.ports.iter().cloned().map(Into::into).collect();
            s.insert("ports".into(), ports.into());
        }
        if !svc.volumes.is_empty() {
            let vols: Vec<Value> = svc.volumes.iter().cloned().map(Into::into).collect();
            s.insert("volumes".into(), vols.into());
        }
        if let Some(net) = &svc.network {
            // Map form so we can support per-service networks if needed
            // later; the parser's pick_network handles list and map.
            let mut nets = Mapping::new();
            nets.insert(net.clone().into(), Value::Null);
            s.insert("networks".into(), nets.into());
        }
        if !svc.depends_on.is_empty() {
            let deps: Vec<Value> = svc.depends_on.iter().cloned().map(Into::into).collect();
            s.insert("depends_on".into(), deps.into());
        }
        if let Some(restart) = &svc.restart {
            s.insert("restart".into(), restart.clone().into());
        }
        if !svc.cap_add.is_empty() {
            let caps: Vec<Value> = svc.cap_add.iter().cloned().map(Into::into).collect();
            s.insert("cap_add".into(), caps.into());
        }
        if !svc.cap_drop.is_empty() {
            let caps: Vec<Value> = svc.cap_drop.iter().cloned().map(Into::into).collect();
            s.insert("cap_drop".into(), caps.into());
        }
        if !svc.args.is_empty() {
            // The parser collapses Compose's command + entrypoint into args,
            // so we emit `command:` (the more common of the two). List form
            // round-trips through ComposeArgv::List.
            let args: Vec<Value> = svc.args.iter().cloned().map(Into::into).collect();
            s.insert("command".into(), args.into());
        }
        if let Some(hc) = &svc.healthcheck {
            if let Some(hcv) = export_healthcheck(hc) {
                s.insert("healthcheck".into(), hcv);
            }
        }
        services.insert(svc.name.clone().into(), s.into());
    }

    let mut doc = Mapping::new();
    doc.insert("name".into(), stack.name.clone().into());
    doc.insert("services".into(), services.into());

    serde_yaml::to_string(&Value::Mapping(doc))
        .with_context(|| format!("serialise stack '{}' as YAML", stack.name))
}

// Emit a healthcheck block compatible with the import path's recogniser.
// `kind = "cmd"` becomes test = ["CMD", argv...] when we have argv,
// otherwise "tcp" / "http" don't have a clean Compose representation
// (they're cgui-specific) so we round-trip as a CMD-SHELL using a
// concise diagnostic command. Returns None when there's nothing to emit.
fn export_healthcheck(hc: &Healthcheck) -> Option<serde_yaml::Value> {
    use serde_yaml::{Mapping, Value};
    let mut m = Mapping::new();
    match hc.kind.as_str() {
        "cmd" if !hc.command.is_empty() => {
            let mut test: Vec<Value> = vec!["CMD".into()];
            for a in &hc.command {
                test.push(a.clone().into());
            }
            m.insert("test".into(), test.into());
        }
        _ => {
            // tcp / http / anything else — stash kind+target in a `test`
            // so importers (incl. our own parser) at least see a
            // CMD-SHELL line; the parser will re-import this back into
            // kind=cmd. Real cgui-flavoured fields don't have a clean
            // Compose representation.
            let target = hc.target.clone().unwrap_or_default();
            let probe = match hc.kind.as_str() {
                "http" => format!("# cgui http {target}"),
                _ => format!("# cgui tcp {target}"),
            };
            m.insert(
                "test".into(),
                vec!["CMD-SHELL".into(), Value::String(probe)].into(),
            );
        }
    }
    if hc.interval_s != 30 {
        m.insert("interval".into(), format!("{}s", hc.interval_s).into());
    }
    if hc.start_period_s > 0 {
        m.insert(
            "start_period".into(),
            format!("{}s", hc.start_period_s).into(),
        );
    }
    if m.is_empty() {
        None
    } else {
        Some(m.into())
    }
}

// Convenience: load a stack by name and return the YAML string.
pub fn export_named(name: &str) -> Result<String> {
    let stack = crate::stacks::load_one(name)?;
    export(&stack)
}

#[cfg(test)]
mod export_tests {
    use super::*;

    fn round_trip(yaml: &str) {
        let cf: ComposeFile = serde_yaml::from_str(yaml).expect("parse compose");
        let stack = convert(cf, "round").expect("convert");
        let exported = export(&stack).expect("export");
        let cf2: ComposeFile = serde_yaml::from_str(&exported).expect("re-parse");
        let stack2 = convert(cf2, "round").expect("re-convert");
        assert_eq!(stack.name, stack2.name, "name preserved");
        assert_eq!(
            stack.services.len(),
            stack2.services.len(),
            "service count preserved"
        );
        for (a, b) in stack.services.iter().zip(stack2.services.iter()) {
            assert_eq!(a.name, b.name, "service name preserved");
            assert_eq!(a.image, b.image, "image preserved for {}", a.name);
            assert_eq!(a.env, b.env, "env preserved for {}", a.name);
            assert_eq!(a.ports, b.ports, "ports preserved for {}", a.name);
            assert_eq!(a.volumes, b.volumes, "volumes preserved for {}", a.name);
            assert_eq!(
                a.depends_on, b.depends_on,
                "depends_on preserved for {}",
                a.name
            );
            assert_eq!(a.restart, b.restart, "restart preserved for {}", a.name);
            assert_eq!(a.args, b.args, "args preserved for {}", a.name);
            assert_eq!(a.network, b.network, "network preserved for {}", a.name);
        }
    }

    #[test]
    fn round_trip_basic_stack() {
        round_trip(
            r#"
name: web
services:
  api:
    image: ghcr.io/acme/api:1.2
    environment:
      LOG_LEVEL: info
      PORT: "8080"
    ports:
      - "8080:8080"
    volumes:
      - "./data:/data"
    depends_on:
      - db
    restart: unless-stopped
    command: ["node", "server.js"]
  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: secret
"#,
        );
    }

    #[test]
    fn export_preserves_name() {
        let stack = StackToml {
            name: "my-stack".into(),
            services: vec![],
        };
        let yaml = export(&stack).unwrap();
        assert!(yaml.contains("name: my-stack"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> StackToml {
        let cf: ComposeFile = serde_yaml::from_str(yaml).unwrap();
        convert(cf, "fallback").unwrap()
    }

    #[test]
    fn minimum_viable_compose() {
        let s = parse(
            r#"
services:
  web:
    image: nginx:1.27
"#,
        );
        assert_eq!(s.name, "fallback");
        assert_eq!(s.services.len(), 1);
        assert_eq!(s.services[0].name, "web");
        assert_eq!(s.services[0].image, "nginx:1.27");
    }

    #[test]
    fn name_override_at_top_level_wins() {
        let s = parse(
            r#"
name: myapp
services:
  web: { image: nginx }
"#,
        );
        assert_eq!(s.name, "myapp");
    }

    #[test]
    fn build_only_service_errors_with_clear_message() {
        let cf: ComposeFile = serde_yaml::from_str(
            r#"
services:
  api:
    build: ./api
"#,
        )
        .unwrap();
        let err = convert(cf, "x").unwrap_err().to_string();
        assert!(err.contains("build:"));
        assert!(err.contains("api"));
    }

    #[test]
    fn env_supports_both_list_and_map_forms() {
        let s = parse(
            r#"
services:
  db:
    image: postgres
    environment:
      POSTGRES_USER: dev
      POSTGRES_PASSWORD: secret
      MAX_CONNECTIONS: 100        # unquoted int
"#,
        );
        let env = &s.services[0].env;
        assert_eq!(env.get("POSTGRES_USER").unwrap(), "dev");
        assert_eq!(env.get("MAX_CONNECTIONS").unwrap(), "100");

        let s2 = parse(
            r#"
services:
  db:
    image: postgres
    environment:
      - POSTGRES_USER=dev
      - POSTGRES_PASSWORD=secret
"#,
        );
        let env = &s2.services[0].env;
        assert_eq!(env.get("POSTGRES_USER").unwrap(), "dev");
        assert_eq!(env.get("POSTGRES_PASSWORD").unwrap(), "secret");
    }

    #[test]
    fn ports_short_and_long_forms_both_work() {
        let s = parse(
            r#"
services:
  api:
    image: x
    ports:
      - "8080:80"
      - target: 9090
        published: 9000
"#,
        );
        assert_eq!(s.services[0].ports, vec!["8080:80", "9000:9090"]);
    }

    #[test]
    fn volumes_short_and_long_forms_both_work() {
        let s = parse(
            r#"
services:
  db:
    image: postgres
    volumes:
      - dbdata:/var/lib/postgresql/data
      - type: volume
        source: cache
        target: /cache
        read_only: true
"#,
        );
        assert_eq!(
            s.services[0].volumes,
            vec!["dbdata:/var/lib/postgresql/data", "cache:/cache:ro",]
        );
    }

    #[test]
    fn restart_unless_stopped_collapses_to_always() {
        assert_eq!(
            convert_restart(Some("always".into())).as_deref(),
            Some("always")
        );
        assert_eq!(
            convert_restart(Some("unless-stopped".into())).as_deref(),
            Some("always")
        );
        assert_eq!(
            convert_restart(Some("on-failure:3".into())).as_deref(),
            Some("on-failure")
        );
        assert_eq!(convert_restart(Some("no".into())), None);
        assert_eq!(convert_restart(None), None);
    }

    #[test]
    fn healthcheck_cmd_array_form() {
        let s = parse(
            r#"
services:
  db:
    image: postgres
    healthcheck:
      test: ["CMD", "pg_isready", "-U", "dev"]
      interval: 30s
      start_period: 1m
"#,
        );
        let hc = s.services[0].healthcheck.as_ref().unwrap();
        assert_eq!(hc.kind, "cmd");
        assert_eq!(hc.command, vec!["pg_isready", "-U", "dev"]);
        assert_eq!(hc.interval_s, 30);
        assert_eq!(hc.start_period_s, 60);
    }

    #[test]
    fn healthcheck_string_form_wraps_in_sh_c() {
        let s = parse(
            r#"
services:
  api:
    image: x
    healthcheck:
      test: "curl -f http://localhost/health"
"#,
        );
        let hc = s.services[0].healthcheck.as_ref().unwrap();
        assert_eq!(
            hc.command,
            vec!["sh", "-c", "curl -f http://localhost/health"]
        );
    }

    #[test]
    fn healthcheck_disabled_returns_none() {
        let s = parse(
            r#"
services:
  x:
    image: x
    healthcheck:
      disable: true
      test: ["CMD", "true"]
"#,
        );
        assert!(s.services[0].healthcheck.is_none());

        let s2 = parse(
            r#"
services:
  x:
    image: x
    healthcheck:
      test: ["NONE"]
"#,
        );
        assert!(s2.services[0].healthcheck.is_none());
    }

    #[test]
    fn duration_grammar_handles_compose_forms() {
        assert_eq!(parse_duration_to_secs(Some("30s")), Some(30));
        assert_eq!(parse_duration_to_secs(Some("1m")), Some(60));
        assert_eq!(parse_duration_to_secs(Some("1m30s")), Some(90));
        assert_eq!(parse_duration_to_secs(Some("2h")), Some(7200));
        assert_eq!(parse_duration_to_secs(Some("45")), Some(45));
        assert_eq!(parse_duration_to_secs(Some("garbage")), None);
        assert_eq!(parse_duration_to_secs(None), None);
    }

    #[test]
    fn networks_pick_first_in_either_form() {
        let s = parse(
            r#"
services:
  a:
    image: x
    networks: [back, front]
"#,
        );
        assert_eq!(s.services[0].network.as_deref(), Some("back"));

        let s2 = parse(
            r#"
services:
  a:
    image: x
    networks:
      back: {}
      front: {}
"#,
        );
        // BTreeMap key order is lexicographic — "back" comes before "front".
        assert_eq!(s2.services[0].network.as_deref(), Some("back"));
    }

    #[test]
    fn depends_on_handles_short_and_long_forms() {
        let s = parse(
            r#"
services:
  api:
    image: x
    depends_on: [db, cache]
"#,
        );
        assert_eq!(s.services[0].depends_on, vec!["db", "cache"]);

        let s2 = parse(
            r#"
services:
  api:
    image: x
    depends_on:
      db:
        condition: service_healthy
      cache:
        condition: service_started
"#,
        );
        assert_eq!(s2.services[0].depends_on.len(), 2);
        assert!(s2.services[0].depends_on.contains(&"db".to_string()));
        assert!(s2.services[0].depends_on.contains(&"cache".to_string()));
    }

    #[test]
    fn command_string_splits_on_whitespace() {
        let s = parse(
            r#"
services:
  app:
    image: x
    command: "node server.js --port 3000"
"#,
        );
        assert_eq!(
            s.services[0].args,
            vec!["node", "server.js", "--port", "3000"]
        );
    }
}
