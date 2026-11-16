use crate::{Attributes, Colors, DetRng, Formation, Player, Role, Team};

const TEAM_PLACES: &[&str] = &[
    "Ash", "Brine", "Cinder", "Dune", "Ember", "Fjord", "Gale", "Harbor", "Juniper", "Kelp",
    "Lumen", "Marrow", "Nettle", "Opal", "Pebble", "Quill",
];
const TEAM_MASCOTS: &[&str] = &[
    "Kestrels", "Otters", "Herons", "Lynxes", "Foxes", "Badgers", "Marmots", "Plovers", "Stoats",
    "Newts", "Wrens", "Hares",
];
const GIVEN: &[&str] = &[
    "Ari", "Bex", "Cato", "Dara", "Elio", "Fenn", "Gia", "Hale", "Iko", "Jory", "Kade", "Lio",
    "Mara", "Nox", "Odel", "Pia", "Quin", "Rhea", "Sol", "Tavi", "Ume", "Vero", "Wick", "Yara",
];
const FAMILY: &[&str] = &[
    "Brook", "Calder", "Drift", "Everly", "Flint", "Garrow", "Holm", "Ivers", "Jessop", "Knoll",
    "Lark", "Mercer", "Norrow", "Orlow", "Penn", "Quarry", "Rowan", "Sable", "Thorne", "Vale",
];

/// Builds an original team from a seed: invented names, colors and ratings.
pub fn generate_team(seed: u64, formation: &Formation) -> Team {
    let mut rng = DetRng::new(seed ^ 0xA5A5_5A5A_1234_4321);
    let name = format!("{} {}", rng.pick(TEAM_PLACES), rng.pick(TEAM_MASCOTS));
    let colors = Colors {
        primary: [
            rng.range(30, 230) as u8,
            rng.range(30, 230) as u8,
            rng.range(30, 230) as u8,
        ],
        secondary: [245, 245, 240],
    };
    let mut numbers: Vec<u8> = (1..=30).collect();
    let players = formation
        .slots()
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            let number = if index == 0 {
                1
            } else {
                let pool: Vec<u8> = numbers.iter().copied().filter(|&n| n > 1).collect();
                *rng.pick(&pool)
            };
            numbers.retain(|&n| n != number);
            Player {
                number,
                name: format!("{} {}", rng.pick(GIVEN), rng.pick(FAMILY)),
                role: slot.role,
                attributes: attributes_for(slot.role, &mut rng),
            }
        })
        .collect();
    Team {
        name,
        colors,
        formation: formation.clone(),
        players,
    }
}

fn attributes_for(role: Role, rng: &mut DetRng) -> Attributes {
    let (pace, power, control, vision) = match role {
        Role::Goalkeeper => (45, 55, 65, 60),
        Role::Defender => (55, 60, 50, 50),
        Role::Midfielder => (60, 50, 62, 65),
        Role::Forward => (68, 66, 58, 55),
    };
    let mut roll = |base: i32| (base + rng.range(0, 24) as i32 - 12).clamp(1, 100) as u8;
    Attributes {
        pace: roll(pace),
        power: roll(power),
        control: roll(control),
        vision: roll(vision),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_team() {
        let f = Formation::balanced(5);
        assert_eq!(generate_team(7, &f), generate_team(7, &f));
        assert_ne!(generate_team(7, &f), generate_team(8, &f));
    }

    #[test]
    fn generated_team_validates() {
        for n in 2..=11 {
            let f = Formation::balanced(n);
            generate_team(u64::from(n), &f).validate(n).unwrap();
        }
    }
}
