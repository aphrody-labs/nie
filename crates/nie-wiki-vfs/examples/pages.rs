//! Prints one character, one skill and one team of the mounted installation (`NIE_GAME_SOURCE`)
//! in the requested languages, then the checks docs/wiki.md asks for.
//!
//! `pages [character-key skill-key team-key]` (defaults: the first record of each table).
//! `NIE_WIKI_LANGS=fr,de` selects the languages (default: all).

use std::collections::BTreeMap;

use nie_wiki_vfs::Wiki;

fn main() {
    let mut args = std::env::args().skip(1);
    let (character, skill, team) = (args.next(), args.next(), args.next());
    let mut wiki = Wiki::open(None).expect("mount the game");
    println!(
        "languages: {}",
        wiki.languages()
            .iter()
            .map(|l| format!("{} ({})", l.folder, l.tag))
            .collect::<Vec<_>>()
            .join(", ")
    );
    if let Ok(langs) = std::env::var("NIE_WIKI_LANGS") {
        let folders: Vec<&str> = langs.split(',').collect();
        wiki.select_languages(&folders).expect("languages");
    }

    let characters = wiki.characters().expect("characters");
    let skills = wiki.skills().expect("skills");
    let teams = wiki.teams().expect("teams");
    println!(
        "characters {} skills {} teams {}",
        characters.len(),
        skills.len(),
        teams.len()
    );

    let pick = characters
        .iter()
        .find(|c| character.as_deref().is_none_or(|k| c.internal_code == k))
        .expect("character");
    println!("{}", serde_json::to_string_pretty(pick).unwrap());
    let pick = skills
        .iter()
        .find(|s| skill.as_deref().is_none_or(|k| s.skill_id_str == k))
        .expect("skill");
    println!("{}", serde_json::to_string_pretty(pick).unwrap());
    let pick = teams
        .iter()
        .find(|t| match team.as_deref() {
            Some(k) => format!("0x{:08X}", t.belong_team_id.0) == k,
            None => {
                t.emblems.values().any(|e| !e.files.is_empty())
                    && !t.affiliated_characters.is_empty()
            }
        })
        .expect("team");
    println!("{}", serde_json::to_string_pretty(pick).unwrap());

    let count = |f: &dyn Fn(&nie_wiki_vfs::Character) -> Option<i64>| {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for c in &characters {
            *m.entry(f(c).map_or("absent".to_owned(), |v| v.to_string()))
                .or_default() += 1;
        }
        m
    };
    println!(
        "gender codes: {:?}",
        count(&|c| c.gender.as_ref().map(|g| g.value))
    );
    println!(
        "main_position codes: {:?}",
        count(&|c| Some(c.main_position.value))
    );
    println!("element codes: {:?}", count(&|c| Some(c.element.value)));
    for code in [0, 1, 2, 4, 5] {
        let names: Vec<String> = characters
            .iter()
            .filter(|c| c.gender.as_ref().is_some_and(|g| g.value == code))
            .filter_map(|c| {
                Some(format!(
                    "{} [{}]",
                    c.name.as_ref()?.text("fr")?,
                    c.internal_code
                ))
            })
            .take(4)
            .collect();
        println!("gender {code}: {names:?}");
    }
    for code in 0..=4 {
        let names: Vec<String> = characters
            .iter()
            .filter(|c| c.main_position.value == code)
            .filter_map(|c| c.name.as_ref()?.text("fr").map(str::to_owned))
            .take(4)
            .collect();
        println!("main_position {code}: {names:?}");
    }
    let unresolved: Vec<String> = teams
        .iter()
        .flat_map(|t| {
            t.emblems
                .iter()
                .filter(|(_, e)| e.files.is_empty())
                .map(|(f, e)| {
                    format!(
                        "{f}={:?} ({})",
                        e.emblem_id,
                        t.name.as_ref().and_then(|n| n.text("fr")).unwrap_or("?")
                    )
                })
        })
        .collect();
    println!("unresolved emblems: {unresolved:?}");
    println!(
        "characters with a name: {}",
        characters.iter().filter(|c| c.name.is_some()).count()
    );
    println!(
        "characters with a team: {}",
        characters.iter().filter(|c| c.team.is_some()).count()
    );
    println!(
        "element label found: {}",
        characters
            .iter()
            .filter(|c| c.element.label.is_some())
            .count()
    );
    println!(
        "skills with a name: {}",
        skills.iter().filter(|s| s.name.is_some()).count()
    );
    println!(
        "skill element label found: {}",
        skills
            .iter()
            .filter(|s| s.element.as_ref().is_some_and(|e| e.label.is_some()))
            .count()
    );
    let emblems: Vec<_> = teams.iter().flat_map(|t| t.emblems.values()).collect();
    println!(
        "emblem ids {} resolved {}",
        emblems.len(),
        emblems.iter().filter(|e| !e.files.is_empty()).count()
    );
    println!(
        "teams named {} with affiliated characters {}",
        teams.iter().filter(|t| t.name.is_some()).count(),
        teams
            .iter()
            .filter(|t| !t.affiliated_characters.is_empty())
            .count()
    );
}
