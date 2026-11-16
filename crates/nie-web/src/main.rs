//! `nie-web`: the Dioxus frontend. The page holds a craft manifest, plays it with the same
//! deterministic simulation as every other frontend (compiled to WebAssembly in the browser) and
//! shows the result.

use dioxus::prelude::*;
use nie_craft::Manifest;

const DEFAULT_MANIFEST: &str = include_str!("../../../games/web-cup.toml");
const STYLE: &str = "body{font-family:system-ui,sans-serif;background:#10151c;color:#e8edf2;margin:0}\
main{max-width:56rem;margin:0 auto;padding:1.5rem}\
textarea{width:100%;height:22rem;background:#0b0f14;color:#cfe3ff;border:1px solid #2b3846;padding:.75rem;font:14px/1.4 ui-monospace,monospace}\
button{background:#2f8f5b;color:#fff;border:0;padding:.55rem 1.1rem;border-radius:.3rem;font-size:1rem;cursor:pointer}\
.err{color:#ff8a80}.score{font-size:1.6rem}table{border-collapse:collapse}td,th{padding:.2rem .8rem;text-align:left}";

fn main() {
    dioxus::launch(App);
}

/// One played match, reduced to what the page shows.
#[derive(Clone, PartialEq)]
struct Row {
    score: String,
    ticks: u32,
    goals: usize,
    checksum: String,
}

#[derive(Clone, PartialEq)]
enum Outcome {
    Idle,
    Failed(String),
    Played { title: String, rows: Vec<Row> },
}

fn play(text: &str, ticks: u32) -> Outcome {
    let played =
        Manifest::from_toml(text).and_then(|m| Ok((m.game.name.clone(), m.play(Some(ticks))?)));
    match played {
        Ok((title, reports)) => Outcome::Played {
            title,
            rows: reports
                .iter()
                .map(|r| Row {
                    score: r.score_line(),
                    ticks: r.ticks,
                    goals: r.goals.len(),
                    checksum: format!("{:016x}", r.checksum),
                })
                .collect(),
        },
        Err(error) => Outcome::Failed(error.to_string()),
    }
}

#[component]
fn App() -> Element {
    let mut text = use_signal(|| DEFAULT_MANIFEST.to_owned());
    let mut ticks = use_signal(|| 1800_u32);
    let mut outcome = use_signal(|| Outcome::Idle);

    rsx! {
        style { "{STYLE}" }
        main {
            h1 { "nie craft" }
            p { "Edit the manifest, then play it. The match runs in this page." }
            textarea {
                value: "{text}",
                oninput: move |event| text.set(event.value()),
            }
            p {
                label { "Ticks " }
                input {
                    r#type: "number",
                    min: "1",
                    max: "216000",
                    value: "{ticks}",
                    oninput: move |event| {
                        if let Ok(value) = event.value().parse() {
                            ticks.set(value);
                        }
                    },
                }
                " "
                button { onclick: move |_| outcome.set(play(&text.read(), ticks())), "Play" }
            }
            match outcome() {
                Outcome::Idle => rsx! {},
                Outcome::Failed(message) => rsx! { p { class: "err", "{message}" } },
                Outcome::Played { title, rows } => rsx! {
                    h2 { "{title}" }
                    table {
                        thead { tr { th { "Match" } th { "Score" } th { "Ticks" } th { "Goals" } th { "Checksum" } } }
                        tbody {
                            for (index, row) in rows.into_iter().enumerate() {
                                tr {
                                    td { "{index + 1}" }
                                    td { class: "score", "{row.score}" }
                                    td { "{row.ticks}" }
                                    td { "{row.goals}" }
                                    td { "{row.checksum}" }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}
