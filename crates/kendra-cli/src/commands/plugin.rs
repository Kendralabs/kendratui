//! Handlers for plugin and marketplace management commands.

use crate::cli::{MarketplaceAction, PluginAction};
use kendra_plugins::manager::PluginManager;
use kendra_plugins::models::PluginScope;
use std::path::Path;

/// Handle marketplace subcommands.
pub fn handle_marketplace(action: MarketplaceAction, working_dir: &Path) {
    let manager = PluginManager::new(Some(working_dir.to_path_buf()));

    match action {
        MarketplaceAction::Add { url, name, branch } => {
            println!("Adding marketplace...");
            match manager.add_marketplace(&url, name.as_deref(), &branch) {
                Ok(info) => {
                    println!(
                        "Successfully added marketplace '{}' tracking branch '{}'",
                        info.name, info.branch
                    );
                }
                Err(e) => {
                    eprintln!("Error: failed to add marketplace: {e}");
                    std::process::exit(1);
                }
            }
        }
        MarketplaceAction::Remove { name } => match manager.remove_marketplace(&name) {
            Ok(_) => {
                println!("Successfully removed marketplace '{}'", name);
            }
            Err(e) => {
                eprintln!("Error: failed to remove marketplace: {e}");
                std::process::exit(1);
            }
        },
        MarketplaceAction::List => match manager.list_marketplaces() {
            Ok(marketplaces) => {
                if marketplaces.is_empty() {
                    println!("No marketplaces registered.");
                    println!("Add one with: kendra marketplace add <url> [name]");
                    return;
                }
                println!("{:<20} {:<60} {:<10}", "NAME", "URL", "BRANCH");
                println!("{}", "-".repeat(92));
                for m in marketplaces {
                    println!("{:<20} {:<60} {:<10}", m.name, m.url, m.branch);
                }
            }
            Err(e) => {
                eprintln!("Error: failed to list marketplaces: {e}");
                std::process::exit(1);
            }
        },
        MarketplaceAction::Sync { name } => {
            if let Some(n) = name {
                println!("Syncing marketplace '{n}'...");
                match manager.sync_marketplace(&n) {
                    Ok(()) => println!("Sync of '{n}' completed successfully."),
                    Err(e) => {
                        eprintln!("Error: sync failed: {e}");
                        std::process::exit(1);
                    }
                }
            } else {
                println!("Syncing all marketplaces...");
                match manager.sync_all_marketplaces() {
                    Ok(results) => {
                        let mut had_error = false;
                        for (name, err) in &results {
                            if let Some(e) = err {
                                eprintln!("  {name}: FAILED — {e}");
                                had_error = true;
                            } else {
                                println!("  {name}: OK");
                            }
                        }
                        if had_error {
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: sync failed: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }
        MarketplaceAction::Search { query } => {
            // Search across all registered marketplaces
            match manager.list_marketplaces() {
                Ok(marketplaces) => {
                    if marketplaces.is_empty() {
                        println!(
                            "No marketplaces registered. Add one with: kendra marketplace add <url>"
                        );
                        return;
                    }
                    let mut all_plugins = Vec::new();
                    for m in &marketplaces {
                        if let Ok(plugins) = manager.search_marketplace(&m.name, &query) {
                            for p in plugins {
                                all_plugins.push((m.name.clone(), p));
                            }
                        }
                    }
                    if all_plugins.is_empty() {
                        println!("No plugins found matching '{query}'.");
                        return;
                    }
                    println!(
                        "{:<20} {:<25} {:<10} {:<40}",
                        "MARKETPLACE", "NAME", "VERSION", "DESCRIPTION"
                    );
                    println!("{}", "-".repeat(97));
                    for (mkt, p) in all_plugins {
                        let desc = if p.description.len() > 37 {
                            format!("{}...", &p.description[..34])
                        } else {
                            p.description.clone()
                        };
                        println!("{:<20} {:<25} {:<10} {:<40}", mkt, p.name, p.version, desc);
                    }
                }
                Err(e) => {
                    eprintln!("Error: search failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        MarketplaceAction::Plugins { name } => match manager.list_marketplace_plugins(&name) {
            Ok(plugins) => {
                if plugins.is_empty() {
                    println!("No plugins found in marketplace '{}'.", name);
                    return;
                }
                println!("{:<25} {:<10} {:<50}", "NAME", "VERSION", "DESCRIPTION");
                println!("{}", "-".repeat(87));
                for p in plugins {
                    let desc = if p.description.len() > 47 {
                        format!("{}...", &p.description[..44])
                    } else {
                        p.description.clone()
                    };
                    println!("{:<25} {:<10} {:<50}", p.name, p.version, desc);
                }
            }
            Err(e) => {
                eprintln!("Error: failed to list marketplace plugins: {e}");
                std::process::exit(1);
            }
        },
    }
}

/// Handle plugin subcommands.
pub fn handle_plugin(action: PluginAction, working_dir: &Path) {
    let manager = PluginManager::new(Some(working_dir.to_path_buf()));

    match action {
        PluginAction::Install {
            name,
            marketplace,
            global,
        } => {
            let scope = if global {
                PluginScope::User
            } else {
                PluginScope::Project
            };
            let scope_str = if global { "global" } else { "project" };
            println!(
                "Installing plugin '{}' from marketplace '{}' ({scope_str} scope)...",
                name, marketplace
            );
            match manager.install_plugin(&name, &marketplace, scope) {
                Ok(cfg) => {
                    println!(
                        "Successfully installed plugin '{}' version '{}'",
                        cfg.name, cfg.version
                    );
                }
                Err(e) => {
                    eprintln!("Error: failed to install plugin: {e}");
                    std::process::exit(1);
                }
            }
        }
        PluginAction::Uninstall {
            name,
            marketplace,
            global,
        } => {
            let scope = if global {
                PluginScope::User
            } else {
                PluginScope::Project
            };
            let scope_str = if global { "global" } else { "project" };
            println!(
                "Uninstalling plugin '{}' from marketplace '{}' ({scope_str} scope)...",
                name, marketplace
            );
            match manager.uninstall_plugin(&name, &marketplace, scope) {
                Ok(_) => {
                    println!("Successfully uninstalled plugin '{}'", name);
                }
                Err(e) => {
                    eprintln!("Error: failed to uninstall plugin: {e}");
                    std::process::exit(1);
                }
            }
        }
        PluginAction::List { scope } => {
            let parsed_scope = match scope.as_deref() {
                Some("global") | Some("user") => Some(PluginScope::User),
                Some("project") | Some("local") => Some(PluginScope::Project),
                None => None,
                Some(other) => {
                    eprintln!("Error: unknown scope '{other}' (expected 'global' or 'project')");
                    std::process::exit(1);
                }
            };

            match manager.list_installed(parsed_scope) {
                Ok(plugins) => {
                    if plugins.is_empty() {
                        println!("No plugins installed.");
                        return;
                    }
                    println!(
                        "{:<25} {:<10} {:<10} {:<10} {:<15}",
                        "NAME", "VERSION", "SCOPE", "STATUS", "MARKETPLACE"
                    );
                    println!("{}", "-".repeat(72));
                    for p in plugins {
                        let scope_str = match p.scope {
                            PluginScope::User => "global",
                            PluginScope::Project => "project",
                        };
                        let status_str = match p.status {
                            kendra_plugins::models::PluginStatus::Installed => "enabled",
                            kendra_plugins::models::PluginStatus::Disabled => "disabled",
                            kendra_plugins::models::PluginStatus::Error(_) => "error",
                        };
                        let marketplace_str = p.marketplace.as_deref().unwrap_or("local");
                        println!(
                            "{:<25} {:<10} {:<10} {:<10} {:<15}",
                            p.name, p.version, scope_str, status_str, marketplace_str
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Error: failed to list installed plugins: {e}");
                    std::process::exit(1);
                }
            }
        }
        PluginAction::Enable {
            name,
            marketplace,
            global,
        } => {
            let scope = if global {
                PluginScope::User
            } else {
                PluginScope::Project
            };
            match manager.enable_plugin(&name, &marketplace, scope) {
                Ok(_) => {
                    println!("Successfully enabled plugin '{}'", name);
                }
                Err(e) => {
                    eprintln!("Error: failed to enable plugin: {e}");
                    std::process::exit(1);
                }
            }
        }
        PluginAction::Disable {
            name,
            marketplace,
            global,
        } => {
            let scope = if global {
                PluginScope::User
            } else {
                PluginScope::Project
            };
            match manager.disable_plugin(&name, &marketplace, scope) {
                Ok(_) => {
                    println!("Successfully disabled plugin '{}'", name);
                }
                Err(e) => {
                    eprintln!("Error: failed to disable plugin: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}
