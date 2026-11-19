//! Gamedata tables decoded generically: every node class (T2B) or list (RDBN) of a file, with
//! the sort-index and `[offset, count]` reference nodes attached to the records they belong to.

use std::collections::{BTreeMap, HashMap};

use nie_data::cfgbin::{Node, Var};
use nie_formats::vfs_read::VfsRead;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::index::find_table;
use crate::model::{ClassInfo, ClassRole, RefItem, RefSlice};

const SORT_INDEX: &str = "__SORT_INDEX";
const REF_INFIX: &str = "_REF_";
const CONTAINER_SUFFIXES: [&str; 3] = ["_LIST_BEG", "_BEGIN", "_BEG"];

/// One stored entry.
#[derive(Debug, Default, Clone)]
pub(crate) struct Rec {
    /// T2B variables.
    pub variables: Vec<Value>,
    /// RDBN named fields.
    pub fields: Option<BTreeMap<String, Value>>,
    /// T2B nested nodes as stored.
    pub children: Vec<Value>,
    pub sort_index: Option<i64>,
    /// target class -> (offset, count)
    pub refs: BTreeMap<String, (i64, i64)>,
}

#[derive(Debug)]
pub(crate) struct Class {
    pub info: ClassInfo,
    pub recs: Vec<Rec>,
}

#[derive(Debug)]
pub(crate) struct Table {
    pub stem: String,
    pub path: String,
    pub format: &'static str,
    pub classes: Vec<Class>,
}

impl Table {
    pub(crate) fn class(&self, name: &str) -> Option<&Class> {
        self.classes.iter().find(|c| c.info.name == name)
    }

    /// The slice `[offset, offset + count)` of `class`, empty when it leaves the class.
    pub(crate) fn slice(&self, class: &str, offset: i64, count: i64) -> RefSlice {
        let items = self
            .class(class)
            .and_then(|target| {
                let start = usize::try_from(offset).ok()?;
                let end = start.checked_add(usize::try_from(count).ok()?)?;
                (end <= target.recs.len()).then(|| {
                    (start..end)
                        .map(|index| RefItem {
                            index,
                            variables: target.recs[index].variables.clone(),
                        })
                        .collect()
                })
            })
            .unwrap_or_default();
        RefSlice {
            class: class.to_owned(),
            offset,
            count,
            items,
        }
    }
}

pub(crate) fn var_value(var: &Var<'_>) -> Value {
    match var.ty {
        "String" => Value::from(var.value),
        "Float" => Value::from(var.as_f64()),
        _ => Value::from(var.as_i64()),
    }
}

fn variables(node: &Node<'_>) -> Vec<Value> {
    (0..node.var_count())
        .filter_map(|i| node.var(i))
        .map(|var| var_value(&var))
        .collect()
}

/// The main class of a container node name (`X_LIST_BEG` gives `X`).
fn container_main(name: &str) -> Option<&str> {
    CONTAINER_SUFFIXES
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
}

/// The class a `<MAIN>_REF_<TARGET>` node slices: the longest proper `_`-prefix `P` of `MAIN`
/// with a class `P_<TARGET>`, else `MAIN_<TARGET>`.
fn ref_target<'a>(main: &str, target_tail: &str, classes: &'a [String]) -> Option<&'a String> {
    let mut cuts: Vec<usize> = main.match_indices('_').map(|(i, _)| i).collect();
    cuts.reverse();
    cuts.into_iter()
        .map(|cut| format!("{}_{target_tail}", &main[..cut]))
        .chain(std::iter::once(format!("{main}_{target_tail}")))
        .find_map(|wanted| classes.iter().find(|class| **class == wanted))
}

fn load_t2b(root: &Value) -> Vec<Class> {
    let entries = root.get("entries").and_then(Value::as_array);
    let mut order: Vec<String> = Vec::new();
    let mut recs: HashMap<String, Vec<Rec>> = HashMap::new();
    let mut container_vars: HashMap<String, Vec<Value>> = HashMap::new();
    // class -> (role, main class it annotates)
    let mut roles: HashMap<String, (ClassRole, Option<String>)> = HashMap::new();
    let mut ref_tails: HashMap<String, String> = HashMap::new();

    for raw in entries.into_iter().flatten() {
        let node = Node::new(raw);
        let main = container_main(node.name()).map(str::to_owned);
        let members: Vec<Node<'_>> = if main.is_some() {
            node.children()
        } else {
            vec![node]
        };
        if let Some(main) = &main {
            container_vars.insert(main.clone(), variables(&node));
        }
        for member in members {
            let name = member.name().to_owned();
            if !recs.contains_key(&name) {
                order.push(name.clone());
            }
            let role = match &main {
                Some(main) if name == SORT_INDEX => (ClassRole::SortIndex, Some(main.clone())),
                Some(main) if name.starts_with(&format!("{main}{REF_INFIX}")) => {
                    ref_tails.insert(
                        name.clone(),
                        name[main.len() + REF_INFIX.len()..].to_owned(),
                    );
                    (ClassRole::Reference, Some(main.clone()))
                }
                _ => (ClassRole::Records, None),
            };
            roles.entry(name.clone()).or_insert(role);
            let children = member
                .children()
                .iter()
                .map(|child| node_json(child))
                .collect();
            recs.entry(name).or_default().push(Rec {
                variables: variables(&member),
                children,
                ..Rec::default()
            });
        }
    }

    // Attach annotations to the records they belong to, when the counts agree.
    let names = order.clone();
    for name in &order {
        let (role, main) = roles[name].clone();
        let Some(main) = main else { continue };
        let count = recs.get(name).map_or(0, Vec::len);
        let main_count = recs.get(&main).map_or(0, Vec::len);
        if count != main_count || main_count == 0 {
            roles.insert(name.clone(), (ClassRole::Records, None));
            continue;
        }
        let annotations = recs.remove(name).unwrap_or_default();
        let target = (role == ClassRole::Reference)
            .then(|| ref_target(&main, &ref_tails[name], &names).cloned())
            .flatten();
        if role == ClassRole::Reference && target.is_none() {
            roles.insert(name.clone(), (ClassRole::Records, None));
            recs.insert(name.clone(), annotations);
            continue;
        }
        let main_recs = recs.get_mut(&main).expect("main class");
        for (rec, annotation) in main_recs.iter_mut().zip(&annotations) {
            match (&role, &target) {
                (ClassRole::SortIndex, _) => {
                    rec.sort_index = annotation.variables.first().and_then(Value::as_i64);
                }
                (_, Some(target)) => {
                    let value = |i: usize| annotation.variables.get(i).and_then(Value::as_i64);
                    if let (Some(offset), Some(count)) = (value(0), value(1)) {
                        rec.refs.insert(target.clone(), (offset, count));
                    }
                }
                _ => {}
            }
        }
        recs.insert(name.clone(), annotations);
        roles.insert(
            name.clone(),
            (role, Some(target.unwrap_or_else(|| main.clone()))),
        );
    }

    order
        .into_iter()
        .map(|name| {
            let (role, target) = roles.remove(&name).unwrap_or((ClassRole::Records, None));
            let list = recs.remove(&name).unwrap_or_default();
            Class {
                info: ClassInfo {
                    entries: list.len(),
                    container_variables: container_vars.remove(&name).unwrap_or_default(),
                    name,
                    role,
                    target,
                    rich_family: None,
                },
                recs: if role == ClassRole::Records {
                    list
                } else {
                    Vec::new()
                },
            }
        })
        .collect()
}

fn node_json(node: &Node<'_>) -> Value {
    serde_json::json!({
        "name": node.name(),
        "variables": variables(node),
        "children": node.children().iter().map(node_json).collect::<Vec<_>>(),
    })
}

fn load_rdbn(root: &Value) -> Vec<Class> {
    root.get("lists")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|list| {
            let name = list
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let recs: Vec<Rec> = list
                .get("values")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|row| Rec {
                    fields: Some(
                        row.as_object()
                            .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                            .unwrap_or_default(),
                    ),
                    ..Rec::default()
                })
                .collect();
            Class {
                info: ClassInfo {
                    name,
                    role: ClassRole::Records,
                    entries: recs.len(),
                    target: None,
                    rich_family: None,
                    container_variables: Vec::new(),
                },
                recs,
            }
        })
        .collect()
}

pub(crate) fn load(vfs: &dyn VfsRead, stem: &str) -> Result<Table> {
    let path = find_table(vfs, stem)?;
    let root =
        nie_explore::game_data::decode_cfgbin(vfs, &path).map_err(|message| Error::Decode {
            path: path.clone(),
            message,
        })?;
    let rdbn = root.get("lists").is_some();
    Ok(Table {
        stem: stem.to_owned(),
        path,
        format: if rdbn { "rdbn" } else { "t2b" },
        classes: if rdbn {
            load_rdbn(&root)
        } else {
            load_t2b(&root)
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_targets_resolve_through_the_longest_shared_prefix() {
        let classes: Vec<String> = [
            "SEASONAL_FIX_TEAM_PASSIVE_MAIN_POS",
            "SEASONAL_FIX_TEAM_PASSIVE_BUILD_TYPE",
            "SEASONAL_CHARA_PARAM_DATA",
            "PASSIVE_SKILL_EFFECT",
            "CHARA_BASE_BATTLE",
        ]
        .map(str::to_owned)
        .to_vec();
        assert_eq!(
            ref_target("SEASONAL_FIX_TEAM_PASSIVE_BUILD_TYPE", "MAIN_POS", &classes)
                .map(String::as_str),
            Some("SEASONAL_FIX_TEAM_PASSIVE_MAIN_POS")
        );
        assert_eq!(
            ref_target("SEASONAL_CHARA_PARAM_TABLE", "DATA", &classes).map(String::as_str),
            Some("SEASONAL_CHARA_PARAM_DATA")
        );
        assert_eq!(
            ref_target("PASSIVE_SKILL_INFO", "EFFECT", &classes).map(String::as_str),
            Some("PASSIVE_SKILL_EFFECT")
        );
        assert_eq!(ref_target("CHARA_BASE_INFO", "NOPE", &classes), None);
        assert_eq!(
            container_main("CHARA_BASE_INFO_LIST_BEG"),
            Some("CHARA_BASE_INFO")
        );
        assert_eq!(container_main("NOUN_INFO_BEGIN"), Some("NOUN_INFO"));
        assert_eq!(container_main("PLAIN"), None);
    }
}
