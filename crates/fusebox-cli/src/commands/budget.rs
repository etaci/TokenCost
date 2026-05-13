//! `fusebox budget` — read / write the per-tenant budget overrides stored
//! in `fusebox.yaml`.
//!
//! The CLI deliberately stays declarative: it edits the YAML file on disk
//! and tells the operator to restart the proxy. Hot-reload is a Phase 2
//! upgrade; baking it in now would force every component to share an
//! arc-swap of the config.

use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use fusebox_core::{Budget, BudgetWindow, Config};
use serde_yaml::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum BudgetCommand {
    /// Set or replace the budget for a tenant.
    Set(SetArgs),
    /// Print all budgets (default + per-tenant overrides).
    List(ListArgs),
    /// Drop the per-tenant override; tenant falls back to the default.
    Clear(ClearArgs),
}

#[derive(Debug, Args)]
pub struct SetArgs {
    /// Tenant id. Use `default` to change the global fallback.
    #[arg(long)]
    pub tenant: String,
    /// Limit, optionally with a `/window` suffix, e.g. `10/day` or `10`.
    /// Acceptable windows: `minute|hour|day|week|month` (or `1m/1h/1d/1w/1mo`).
    #[arg(long)]
    pub limit: String,
    /// Window to apply when `--limit` doesn't carry one. Defaults to `day`.
    #[arg(long, default_value = "day")]
    pub window: String,
    /// Optional human label that'll show up in the dashboard.
    #[arg(long)]
    pub label: Option<String>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Path to fusebox.yaml; defaults to `./fusebox.yaml`.
    #[arg(long)]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ClearArgs {
    /// Tenant id whose override to remove.
    #[arg(long)]
    pub tenant: String,
}

pub async fn run(cmd: BudgetCommand, global_config: Option<PathBuf>) -> Result<()> {
    match cmd {
        BudgetCommand::Set(a) => set(a, global_config),
        BudgetCommand::List(a) => list(a.path.or(global_config)),
        BudgetCommand::Clear(a) => clear(a, global_config),
    }
}

fn config_path(explicit: Option<PathBuf>) -> PathBuf {
    explicit.unwrap_or_else(|| PathBuf::from("fusebox.yaml"))
}

fn load_or_default(path: &Path) -> Result<Value> {
    if path.exists() {
        let body = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        let v: Value = serde_yaml::from_str(&body).context("parse fusebox.yaml")?;
        return Ok(v);
    }
    // Synthesize a starter doc from the typed defaults so even a missing
    // file produces a sensible YAML on first write.
    let cfg = Config::default();
    Ok(serde_yaml::to_value(&cfg)?)
}

fn set(args: SetArgs, global_config: Option<PathBuf>) -> Result<()> {
    let path = config_path(global_config);
    let mut doc = load_or_default(&path)?;
    let (limit, window) = parse_limit(&args.limit, &args.window)?;
    let budget = Budget {
        limit_usd: limit,
        window,
        label: args.label.clone(),
    };
    apply_budget(&mut doc, &args.tenant, &budget)?;

    let body = serde_yaml::to_string(&doc)?;
    std::fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
    println!(
        "✔ {} budget for {} → ${} per {}",
        if args.tenant == "default" { "default" } else { "tenant" },
        args.tenant,
        limit,
        window.as_label()
    );
    println!("  restart `fusebox start` for the change to take effect.");
    Ok(())
}

fn list(path: Option<PathBuf>) -> Result<()> {
    let actual = path.unwrap_or_else(|| PathBuf::from("fusebox.yaml"));
    let cfg = if actual.exists() {
        Config::from_yaml_file(&actual).map_err(|e| anyhow!(e.to_string()))?
    } else {
        Config::default()
    };
    let pol = &cfg.policy;
    println!("default            : ${:.2} per {} {}",
        pol.default_budget.limit_usd,
        pol.default_budget.window.as_label(),
        pol.default_budget.label.as_deref().map(|l| format!("({l})")).unwrap_or_default()
    );
    if pol.tenant_budgets.is_empty() {
        println!("(no per-tenant overrides)");
        return Ok(());
    }
    let mut keys: Vec<&String> = pol.tenant_budgets.keys().collect();
    keys.sort();
    for k in keys {
        let budgets = &pol.tenant_budgets[k];
        for b in budgets {
            println!(
                "{:<18} : ${:.2} per {} {}",
                k,
                b.limit_usd,
                b.window.as_label(),
                b.label.as_deref().map(|l| format!("({l})")).unwrap_or_default()
            );
        }
    }
    Ok(())
}

fn clear(args: ClearArgs, global_config: Option<PathBuf>) -> Result<()> {
    let path = config_path(global_config);
    if !path.exists() {
        return Err(anyhow!("{} does not exist", path.display()));
    }
    let mut doc = load_or_default(&path)?;
    let removed = remove_tenant_budget(&mut doc, &args.tenant);
    if !removed {
        return Err(anyhow!("no override found for tenant `{}`", args.tenant));
    }
    let body = serde_yaml::to_string(&doc)?;
    std::fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
    println!("✔ cleared budget override for tenant {}", args.tenant);
    Ok(())
}

/// Parse `--limit` with optional `/window` suffix. Returns (usd, window).
fn parse_limit(limit: &str, window: &str) -> Result<(f64, BudgetWindow)> {
    let trimmed = limit.trim().trim_start_matches('$');
    if let Some((amount, win)) = trimmed.split_once('/') {
        let amount: f64 = amount
            .parse()
            .with_context(|| format!("invalid amount `{amount}`"))?;
        let w = parse_window(win)?;
        Ok((amount, w))
    } else {
        let amount: f64 = trimmed
            .parse()
            .with_context(|| format!("invalid amount `{trimmed}`"))?;
        let w = parse_window(window)?;
        Ok((amount, w))
    }
}

fn parse_window(s: &str) -> Result<BudgetWindow> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "minute" | "min" | "1m" | "m" => BudgetWindow::Minute,
        "hour" | "1h" | "h" => BudgetWindow::Hour,
        "day" | "1d" | "d" => BudgetWindow::Day,
        "week" | "1w" | "w" => BudgetWindow::Week,
        "month" | "1mo" | "mo" => BudgetWindow::Month,
        other => return Err(anyhow!("unknown window `{other}` — try minute/hour/day/week/month")),
    })
}

/// Mutate the YAML `Value` to set a budget. Tenant `default` rewrites
/// `policy.default_budget`; everything else lands in `policy.tenant_budgets`.
fn apply_budget(doc: &mut Value, tenant: &str, budget: &Budget) -> Result<()> {
    use serde_yaml::Mapping;

    let policy = ensure_mapping(doc, "policy");
    let budget_yaml = serde_yaml::to_value(budget)?;
    if tenant == "default" {
        policy.insert(Value::String("default_budget".to_string()), budget_yaml);
        return Ok(());
    }
    let tenants_entry = policy
        .entry(Value::String("tenant_budgets".to_string()))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let tenants = tenants_entry
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("policy.tenant_budgets is not a mapping"))?;
    let arr = vec![budget_yaml];
    tenants.insert(
        Value::String(tenant.to_string()),
        Value::Sequence(arr),
    );
    Ok(())
}

fn remove_tenant_budget(doc: &mut Value, tenant: &str) -> bool {
    let Some(policy) = doc
        .as_mapping_mut()
        .and_then(|m| m.get_mut(Value::String("policy".to_string())))
    else {
        return false;
    };
    let Some(tenants) = policy
        .as_mapping_mut()
        .and_then(|m| m.get_mut(Value::String("tenant_budgets".to_string())))
    else {
        return false;
    };
    if let Some(map) = tenants.as_mapping_mut() {
        map.remove(Value::String(tenant.to_string())).is_some()
    } else {
        false
    }
}

/// Walk into `doc[key]`, create as a mapping if missing, return mut ref.
fn ensure_mapping<'a>(doc: &'a mut Value, key: &str) -> &'a mut serde_yaml::Mapping {
    use serde_yaml::Mapping;
    if !doc.is_mapping() {
        *doc = Value::Mapping(Mapping::new());
    }
    let map = doc.as_mapping_mut().expect("just made into mapping");
    map.entry(Value::String(key.to_string()))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    map.get_mut(Value::String(key.to_string()))
        .and_then(|v| v.as_mapping_mut())
        .expect("inserted as mapping above")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_limit_with_window_suffix() {
        let (amt, win) = parse_limit("10/day", "hour").unwrap();
        assert!((amt - 10.0).abs() < 1e-9);
        assert_eq!(win, BudgetWindow::Day);
    }

    #[test]
    fn parses_limit_without_suffix() {
        let (amt, win) = parse_limit("$5.50", "1h").unwrap();
        assert!((amt - 5.50).abs() < 1e-9);
        assert_eq!(win, BudgetWindow::Hour);
    }

    #[test]
    fn unknown_window_errors() {
        assert!(parse_limit("1/year", "day").is_err());
    }

    #[test]
    fn applies_default_budget() {
        let mut v: Value = serde_yaml::from_str("policy: {}").unwrap();
        let b = Budget::new(7.0, BudgetWindow::Day);
        apply_budget(&mut v, "default", &b).unwrap();
        let back: Config = serde_yaml::from_value(v).unwrap();
        assert!((back.policy.default_budget.limit_usd - 7.0).abs() < 1e-9);
    }

    #[test]
    fn applies_tenant_budget() {
        let mut v: Value = serde_yaml::from_str("policy: {}").unwrap();
        let b = Budget::new(2.0, BudgetWindow::Hour);
        apply_budget(&mut v, "alice", &b).unwrap();
        let back: Config = serde_yaml::from_value(v).unwrap();
        let bs = back
            .policy
            .tenant_budgets
            .get("alice")
            .expect("alice present");
        assert_eq!(bs.len(), 1);
        assert!((bs[0].limit_usd - 2.0).abs() < 1e-9);
    }
}
