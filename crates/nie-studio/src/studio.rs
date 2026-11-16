use std::path::PathBuf;

use eframe::egui;
use nie_craft::{ArtStyle, CameraStyle, Controller, Frontend, Manifest, Mode, Series, TeamSpec};
use nie_model::Formation;
use nie_sim::MatchReport;

pub struct Studio {
    files: Vec<PathBuf>,
    open: Option<usize>,
    manifest: Option<Manifest>,
    formations: [String; 2],
    ticks: u32,
    reports: Vec<MatchReport>,
    status: String,
}

impl Studio {
    pub fn new(games: PathBuf, first: Option<PathBuf>) -> Self {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&games)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "toml"))
            .collect();
        files.sort();
        if let Some(path) = first.filter(|p| !files.contains(p)) {
            files.insert(0, path);
        }
        let mut studio = Self {
            files,
            open: None,
            manifest: None,
            formations: Default::default(),
            ticks: 600,
            reports: Vec::new(),
            status: String::new(),
        };
        if !studio.files.is_empty() {
            studio.select(0);
        }
        studio
    }

    fn select(&mut self, index: usize) {
        self.open = Some(index);
        self.reports.clear();
        match Manifest::load(&self.files[index]) {
            Ok(manifest) => {
                self.formations = [&manifest.teams.home, &manifest.teams.away].map(|t| {
                    t.formation
                        .as_ref()
                        .map(Formation::to_string)
                        .unwrap_or_default()
                });
                self.manifest = Some(manifest);
                self.status = format!("opened {}", self.files[index].display());
            }
            Err(error) => {
                self.manifest = None;
                self.status = error.to_string();
            }
        }
    }

    /// Writes the form buffers back into the typed manifest and validates it.
    fn commit(&mut self) -> Result<&Manifest, String> {
        let manifest = self.manifest.as_mut().ok_or("no manifest open")?;
        for (spec, text) in [&mut manifest.teams.home, &mut manifest.teams.away]
            .into_iter()
            .zip(&self.formations)
        {
            spec.formation = match text.trim() {
                "" => None,
                text => Some(Formation::try_from(text.to_owned()).map_err(|e| e.to_string())?),
            };
        }
        match manifest.game.mode {
            Mode::Series => {
                manifest.series.get_or_insert(Series { matches: 3 });
            }
            Mode::Exhibition => manifest.series = None,
        }
        manifest.validate().map_err(|e| e.to_string())?;
        Ok(manifest)
    }

    fn save(&mut self) {
        let Some(index) = self.open else { return };
        self.status = match self.commit().map(Manifest::to_toml) {
            Ok(Ok(text)) => match std::fs::write(&self.files[index], text) {
                Ok(()) => format!("saved {}", self.files[index].display()),
                Err(error) => error.to_string(),
            },
            Ok(Err(error)) => error.to_string(),
            Err(error) => error,
        };
    }

    fn simulate(&mut self) {
        let ticks = self.ticks;
        match self.commit().map(|m| m.play(Some(ticks))) {
            Ok(Ok(reports)) => {
                self.status = format!("simulated {} match(es), {ticks} ticks each", reports.len());
                self.reports = reports;
            }
            Ok(Err(error)) => self.status = error.to_string(),
            Err(error) => self.status = error,
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        let Some(manifest) = self.manifest.as_mut() else {
            ui.label("Select a game on the left.");
            return;
        };
        ui.heading(manifest.game.name.clone());
        egui::Grid::new("game").num_columns(2).show(ui, |ui| {
            ui.label("Id");
            ui.text_edit_singleline(&mut manifest.game.id);
            ui.end_row();
            ui.label("Name");
            ui.text_edit_singleline(&mut manifest.game.name);
            ui.end_row();
            ui.label("Version");
            ui.text_edit_singleline(&mut manifest.game.version);
            ui.end_row();
            ui.label("Mode");
            egui::ComboBox::from_id_salt("mode")
                .selected_text(format!("{:?}", manifest.game.mode))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut manifest.game.mode, Mode::Exhibition, "Exhibition");
                    ui.selectable_value(&mut manifest.game.mode, Mode::Series, "Series");
                });
            ui.end_row();
            ui.label("Frontend");
            egui::ComboBox::from_id_salt("frontend")
                .selected_text(format!("{:?}", manifest.game.frontend))
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (Frontend::Bevy3d, "Bevy 3D"),
                        (Frontend::Studio, "Studio"),
                        (Frontend::DioxusWeb, "Dioxus web"),
                        (Frontend::DioxusDesktop, "Dioxus desktop"),
                    ] {
                        ui.selectable_value(&mut manifest.game.frontend, value, label);
                    }
                });
            ui.end_row();
        });

        ui.separator();
        ui.strong("Rules");
        let rules = &mut manifest.rules;
        egui::Grid::new("rules").num_columns(2).show(ui, |ui| {
            ui.label("Players per side");
            ui.add(egui::DragValue::new(&mut rules.players_per_side).range(2..=11));
            ui.end_row();
            ui.label("Tick rate");
            ui.add(egui::DragValue::new(&mut rules.tick_rate).range(10..=240));
            ui.end_row();
            ui.label("Match seconds");
            ui.add(egui::DragValue::new(&mut rules.match_seconds).range(5..=5400));
            ui.end_row();
            ui.label("Pitch length");
            ui.add(egui::DragValue::new(&mut rules.pitch_length).range(20.0..=120.0));
            ui.end_row();
            ui.label("Pitch width");
            ui.add(egui::DragValue::new(&mut rules.pitch_width).range(12.0..=80.0));
            ui.end_row();
            ui.label("Goal width");
            ui.add(egui::DragValue::new(&mut rules.goal_width).range(2.0..=14.0));
            ui.end_row();
            ui.label("Seed");
            ui.add(egui::DragValue::new(&mut rules.seed));
            ui.end_row();
        });
        if let Some(series) = manifest.series.as_mut() {
            ui.horizontal(|ui| {
                ui.label("Series matches");
                ui.add(egui::DragValue::new(&mut series.matches).range(1..=21));
            });
        }

        ui.separator();
        ui.strong("Art");
        egui::Grid::new("art").num_columns(2).show(ui, |ui| {
            ui.label("Style");
            egui::ComboBox::from_id_salt("style")
                .selected_text(format!("{:?}", manifest.art.style))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut manifest.art.style, ArtStyle::Lowpoly, "Lowpoly");
                    ui.selectable_value(&mut manifest.art.style, ArtStyle::Flat, "Flat");
                    ui.selectable_value(&mut manifest.art.style, ArtStyle::Toon, "Toon");
                });
            ui.end_row();
            ui.label("Camera");
            egui::ComboBox::from_id_salt("camera")
                .selected_text(format!("{:?}", manifest.art.camera))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut manifest.art.camera,
                        CameraStyle::Broadcast,
                        "Broadcast",
                    );
                    ui.selectable_value(&mut manifest.art.camera, CameraStyle::TopDown, "Top down");
                });
            ui.end_row();
            ui.label("Pitch color");
            ui.color_edit_button_srgb(&mut manifest.art.pitch_color);
            ui.end_row();
        });

        ui.separator();
        ui.strong("Teams");
        let [home_formation, away_formation] = &mut self.formations;
        for (label, spec, formation) in [
            ("Home", &mut manifest.teams.home, home_formation),
            ("Away", &mut manifest.teams.away, away_formation),
        ] {
            team_row(ui, label, spec, formation);
        }

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                self.save();
            }
            if ui.button("Validate").clicked() {
                self.status = match self.commit() {
                    Ok(_) => "manifest is valid".to_owned(),
                    Err(error) => error,
                };
            }
            ui.label("Ticks");
            ui.add(egui::DragValue::new(&mut self.ticks).range(1..=216_000));
            if ui.button("Simulate").clicked() {
                self.simulate();
            }
        });
        for (index, report) in self.reports.iter().enumerate() {
            ui.label(format!(
                "Match {}: {} after {} ticks, {} goal(s), checksum {:016x}",
                index + 1,
                report.score_line(),
                report.ticks,
                report.goals.len(),
                report.checksum
            ));
        }
    }
}

fn team_row(ui: &mut egui::Ui, label: &str, spec: &mut TeamSpec, formation: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut name = spec.name.clone().unwrap_or_default();
        if ui
            .add(
                egui::TextEdit::singleline(&mut name)
                    .hint_text("generated name")
                    .desired_width(160.0),
            )
            .changed()
        {
            spec.name = (!name.is_empty()).then_some(name);
        }
        ui.label("Formation");
        ui.add(
            egui::TextEdit::singleline(formation)
                .hint_text("balanced")
                .desired_width(70.0),
        );
        ui.label("Seed");
        let mut seed = spec.seed.unwrap_or_default();
        if ui.add(egui::DragValue::new(&mut seed)).changed() {
            spec.seed = Some(seed);
        }
        egui::ComboBox::from_id_salt((label, "controller"))
            .selected_text(format!("{:?}", spec.controller))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut spec.controller, Controller::Ai, "Ai");
                ui.selectable_value(&mut spec.controller, Controller::Human, "Human");
            });
    });
}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(&self.status);
        });
        egui::Panel::left("games").resizable(true).show(ui, |ui| {
            ui.heading("Games");
            let mut clicked = None;
            for (index, path) in self.files.iter().enumerate() {
                let name = path
                    .file_stem()
                    .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
                if ui
                    .selectable_label(self.open == Some(index), name)
                    .clicked()
                {
                    clicked = Some(index);
                }
            }
            if let Some(index) = clicked {
                self.select(index);
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.editor(ui));
        });
    }
}
