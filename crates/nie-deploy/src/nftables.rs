//! The host firewall through the kernel's nf_tables netlink API (`crabtables`, pure Rust, no C
//! library, no `nft` process). Linux only; elsewhere every call reports that.

#[cfg(target_os = "linux")]
pub use linux::{apply, delete_commented_rules, delete_table, has_set};

#[cfg(not(target_os = "linux"))]
pub use other::{apply, delete_commented_rules, delete_table, has_set};

#[cfg(not(target_os = "linux"))]
mod other {
    use anyhow::Result;

    fn unsupported<T>() -> Result<T> {
        anyhow::bail!("nftables needs a Linux kernel")
    }

    pub fn apply(_script: &str) -> Result<()> {
        unsupported()
    }

    pub fn delete_commented_rules(_table: &str, _chain: &str, _comment: &str) -> Result<usize> {
        unsupported()
    }

    pub fn delete_table(_table: &str) -> Result<()> {
        unsupported()
    }

    pub fn has_set(_table: &str, _set: &str) -> bool {
        false
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use anyhow::{Context, Result, anyhow};
    use crabtables::lang::{compile::Compiler, parse};
    use crabtables::{Batch, Family, Socket};

    /// `inet fleet` -> (`Family::Inet`, `fleet`), the spelling `nft` uses.
    pub(super) fn split_table(spec: &str) -> Result<(Family, String)> {
        let (family, name) = spec
            .split_once(char::is_whitespace)
            .map(|(f, n)| (f, n.trim()))
            .ok_or_else(|| anyhow!("table `{spec}`: expected `<family> <name>`"))?;
        let family = [Family::Inet, Family::Ipv4, Family::Ipv6, Family::Arp, Family::Bridge, Family::Netdev]
            .into_iter()
            .find(|f| f.name() == family)
            .ok_or_else(|| anyhow!("table `{spec}`: unknown family `{family}`"))?;
        Ok((family, name.to_owned()))
    }

    /// Compile an nft script against the live ruleset, validate it (a dry run) and commit it as
    /// one atomic transaction.
    pub fn apply(script: &str) -> Result<()> {
        let commands = parse::parse(script).map_err(|e| anyhow!("nft script: {e}"))?;
        let mut socket = Socket::new().context("open the nf_tables netlink socket")?;
        let mut compiler = Compiler::new();
        compiler.learn(&socket.list_ruleset(Family::Unspec).context("read the ruleset")?);
        compiler.compile(&commands).map_err(|e| anyhow!("nft script: {e}"))?;
        let (batch, _) = compiler.into_batch();
        socket.check(&batch).context("nf_tables rejected the script")?;
        socket.commit(&batch).context("nf_tables commit")?;
        Ok(())
    }

    /// Delete, in one transaction, every rule of `table`'s `chain` that carries `comment`.
    /// Returns how many were deleted.
    pub fn delete_commented_rules(table: &str, chain: &str, comment: &str) -> Result<usize> {
        let (family, name) = split_table(table)?;
        let mut socket = Socket::new().context("open the nf_tables netlink socket")?;
        let ruleset = socket.list_ruleset(family).context("read the ruleset")?;
        let mut batch = Batch::new();
        let mut count = 0;
        for rule in ruleset.chain_rules(family, &name, chain).filter(|r| r.comment() == Some(comment)) {
            let handle = rule.handle.context("the kernel listed a rule without a handle")?;
            batch.delete_rule(family, &name, chain, handle);
            count += 1;
        }
        if count > 0 {
            socket.commit(&batch).context("delete the rules")?;
        }
        Ok(count)
    }

    /// Delete a table with everything in it; an error when it does not exist.
    pub fn delete_table(table: &str) -> Result<()> {
        let (family, name) = split_table(table)?;
        let mut batch = Batch::new();
        batch.delete_table(family, &name);
        Socket::new()
            .context("open the nf_tables netlink socket")?
            .commit(&batch)
            .with_context(|| format!("delete table {table}"))?;
        Ok(())
    }

    /// Whether the named set exists in the table.
    pub fn has_set(table: &str, set: &str) -> bool {
        let Ok((family, name)) = split_table(table) else {
            return false;
        };
        Socket::new()
            .and_then(|mut s| s.list_ruleset(family))
            .is_ok_and(|rs| {
                rs.sets.iter().any(|(s, _)| s.family == family && s.table == name && s.name == set)
            })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::cloudflare::{nft_insert_rules, nft_script};
        use crabtables::lang::{list_ruleset, rule_text};

        /// Enter a fresh network namespace for this thread. `None` (with the reason printed)
        /// when the process may not create one.
        fn netns() -> Option<()> {
            match nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET) {
                Ok(()) => Some(()),
                Err(e) => {
                    eprintln!("skipped: no network namespace ({e}); run as root or under `unshare -rn`");
                    None
                }
            }
        }

        fn ranges() -> (Vec<String>, Vec<String>) {
            (
                vec!["173.245.48.0/20".into(), "103.21.244.0/22".into()],
                vec!["2400:cb00::/32".into(), "2606:4700::/32".into()],
            )
        }

        fn listing() -> String {
            let mut socket = Socket::new().expect("netlink socket");
            list_ruleset(&socket.list_ruleset(Family::Unspec).expect("list"), &Default::default())
        }

        /// The `nft` binary's own reading of the ruleset, when it exists (only to cross-check).
        fn nft_listing() -> Option<String> {
            let out = std::process::Command::new("nft").args(["list", "ruleset"]).output().ok()?;
            out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
        }

        #[test]
        fn own_table_firewall_roundtrip() {
            let Some(()) = netns() else { return };
            let (v4, v6) = ranges();
            apply(&nft_script(2053, &v4, &v6, None)).expect("apply");
            apply(&nft_script(2053, &v4, &v6, None)).expect("apply is idempotent");
            assert!(has_set("inet nie_edge", "nie_cf4") && has_set("inet nie_edge", "nie_cf6"));
            assert!(!has_set("inet nie_edge", "nie_cf9"));

            let mut socket = Socket::new().unwrap();
            let rs = socket.list_ruleset(Family::Inet).unwrap();
            let rules: Vec<_> = rs.chain_rules(Family::Inet, "nie_edge", "input").collect();
            assert_eq!(rules.len(), 3, "two accepts and the drop");
            assert!(rules.iter().all(|r| r.comment() == Some("nie-edge")));
            let text = rule_text(&rs, rules[0], &Default::default());
            assert!(text.contains("tcp dport 2053") && text.contains("@nie_cf4") && text.contains("accept"), "{text}");
            let text_all = listing();
            for cidr in v4.iter().chain(&v6) {
                assert!(text_all.contains(cidr.as_str()), "{cidr} missing from\n{text_all}");
            }
            let elements = |name: &str| {
                rs.sets.iter().find(|(s, _)| s.name == name).map(|(_, e)| e.iter().filter(|e| !e.is_interval_end()).count())
            };
            assert_eq!((elements("nie_cf4"), elements("nie_cf6")), (Some(2), Some(2)));

            if let Some(nft) = nft_listing() {
                let lines = |t: &str| t.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>();
                assert_eq!(lines(&nft), lines(&text_all), "nft and netlink disagree");
                eprintln!("nft list ruleset agrees with the netlink listing ({} lines)", nft.lines().count());
            } else {
                eprintln!("nft binary absent: cross-check skipped");
            }

            assert_eq!(delete_commented_rules("inet nie_edge", "input", "nie-edge").unwrap(), 3);
            assert_eq!(delete_commented_rules("inet nie_edge", "input", "nie-edge").unwrap(), 0);
            delete_table("inet nie_edge").expect("delete table");
            assert!(!has_set("inet nie_edge", "nie_cf4"));
            assert!(!listing().contains("nie_edge"));
            assert!(delete_table("inet nie_edge").is_err(), "a missing table is an error");
        }

        #[test]
        fn shared_table_rules_are_inserted_and_removed_by_comment() {
            let Some(()) = netns() else { return };
            let (v4, v6) = ranges();
            apply(
                "table inet fleet { chain input { type filter hook input priority 0; policy drop; \
                 tcp dport 22 accept; }; }",
            )
            .expect("fleet table");
            let table = Some("inet fleet");
            for _ in 0..2 {
                apply(&nft_script(2053, &v4, &v6, table)).expect("sets");
                delete_commented_rules("inet fleet", "input", "nie-edge").expect("clear");
                apply(&nft_insert_rules(2053, "inet fleet").join("\n")).expect("insert");
            }
            let mut socket = Socket::new().unwrap();
            let rs = socket.list_ruleset(Family::Inet).unwrap();
            let rules: Vec<_> = rs.chain_rules(Family::Inet, "fleet", "input").collect();
            assert_eq!(rules.len(), 3, "ssh plus the two accepts, no duplicates");
            assert_eq!(rules.iter().filter(|r| r.comment() == Some("nie-edge")).count(), 2);
            assert!(rules[..2].iter().all(|r| r.comment() == Some("nie-edge")), "inserted before the ssh rule");
            assert!(rules[2].comment().is_none() && rule_text(&rs, rules[2], &Default::default()).contains("dport 22"));
            let accepts: String = rules[..2].iter().map(|r| rule_text(&rs, r, &Default::default())).collect();
            assert!(accepts.contains("@nie_cf4") && accepts.contains("@nie_cf6"), "{accepts}");
            assert!(has_set("inet fleet", "nie_cf6"));

            assert_eq!(delete_commented_rules("inet fleet", "input", "nie-edge").unwrap(), 2);
            let rs = socket.list_ruleset(Family::Inet).unwrap();
            assert_eq!(rs.chain_rules(Family::Inet, "fleet", "input").count(), 1);
        }

        #[test]
        fn bad_input_is_reported() {
            assert!(split_table("inet").is_err());
            assert!(split_table("nope fleet").is_err());
            assert_eq!(split_table("inet  fleet").unwrap(), (Family::Inet, "fleet".to_owned()));
            assert!(apply("this is not nft").is_err());
        }
    }
}
