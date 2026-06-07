#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EduSnapshot {
    pub frame: u64,
    pub layout_id: Option<u16>,
    pub party_count: Option<u8>,
    pub party_species: Option<u16>,
    pub starter_var: Option<u16>,
    pub pokemon_get: Option<bool>,
    pub rescued_birch: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EduObjective {
    pub id: &'static str,
    pub title: &'static str,
    pub prompt: &'static str,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct EduTracker {
    objectives: Vec<EduObjective>,
}

impl Default for EduTracker {
    fn default() -> Self {
        Self::emerald_onboarding()
    }
}

impl EduTracker {
    pub fn emerald_onboarding() -> Self {
        Self {
            objectives: vec![
                EduObjective {
                    id: "moving_truck",
                    title: "Wake up in the moving truck",
                    prompt: "Orientation: notice where the story starts before rushing inputs.",
                    completed_at: None,
                },
                EduObjective {
                    id: "littleroot",
                    title: "Reach Littleroot Town",
                    prompt: "Map-reading: identify the home town and the exits from it.",
                    completed_at: None,
                },
                EduObjective {
                    id: "route101",
                    title: "Reach Route 101",
                    prompt: "Navigation puzzle: leave town and find the north route.",
                    completed_at: None,
                },
                EduObjective {
                    id: "rescue_birch",
                    title: "Rescue Professor Birch",
                    prompt: "Causal reasoning: the emergency changes the available choices.",
                    completed_at: None,
                },
                EduObjective {
                    id: "starter_acquired",
                    title: "Acquire a starter",
                    prompt:
                        "Decision reflection: compare starter type, tradeoffs, and next objective.",
                    completed_at: None,
                },
            ],
        }
    }

    pub fn update_from_summary(&mut self, summary: &str, frame: u64) -> Vec<&EduObjective> {
        let snapshot = EduSnapshot::from_debug_summary(summary, frame);
        self.update(&snapshot)
    }

    pub fn update(&mut self, snapshot: &EduSnapshot) -> Vec<&EduObjective> {
        let mut completed = Vec::new();
        for objective in &mut self.objectives {
            if objective.completed_at.is_some() {
                continue;
            }
            let done = match objective.id {
                "moving_truck" => snapshot.layout_id == Some(0x00ed),
                "littleroot" => snapshot.layout_id == Some(0x000a),
                "route101" => snapshot.layout_id == Some(0x0011),
                "rescue_birch" => snapshot.rescued_birch == Some(true),
                "starter_acquired" => {
                    snapshot.party_count.unwrap_or(0) >= 1
                        && snapshot.party_species.unwrap_or(0) != 0
                        && snapshot.starter_var.is_some_and(|starter| starter > 0)
                }
                _ => false,
            };
            if done {
                objective.completed_at = Some(snapshot.frame);
                completed.push(objective as &EduObjective);
            }
        }
        completed
    }

    pub fn print_summary(&self) {
        let completed = self
            .objectives
            .iter()
            .filter(|objective| objective.completed_at.is_some())
            .count();
        println!(
            "EDU summary: {completed}/{} objectives completed",
            self.objectives.len()
        );
        for objective in &self.objectives {
            match objective.completed_at {
                Some(frame) => println!(
                    "EDU objective={} status=complete frame={} title=\"{}\" prompt=\"{}\"",
                    objective.id, frame, objective.title, objective.prompt
                ),
                None => println!(
                    "EDU objective={} status=pending title=\"{}\" prompt=\"{}\"",
                    objective.id, objective.title, objective.prompt
                ),
            }
        }
    }

    pub fn objectives(&self) -> &[EduObjective] {
        &self.objectives
    }
}

impl EduSnapshot {
    pub fn from_debug_summary(summary: &str, frame: u64) -> Self {
        Self {
            frame,
            layout_id: find_hex_field(summary, "layoutId="),
            party_count: find_decimal_field(summary, "PARTY count=")
                .and_then(|value| value.try_into().ok()),
            party_species: find_decimal_field(summary, "species=")
                .and_then(|value| value.try_into().ok()),
            starter_var: find_decimal_field(summary, "starter=")
                .and_then(|value| value.try_into().ok()),
            pokemon_get: find_bool_field(summary, "pokemon_get="),
            rescued_birch: find_bool_field(summary, "rescued_birch="),
        }
    }
}

fn find_hex_field(summary: &str, marker: &str) -> Option<u16> {
    let value = summary.split(marker).nth(1)?.split_whitespace().next()?;
    u16::from_str_radix(value, 16).ok()
}

fn find_decimal_field(summary: &str, marker: &str) -> Option<u32> {
    let value = summary.split(marker).nth(1)?.split_whitespace().next()?;
    value.parse().ok()
}

fn find_bool_field(summary: &str, marker: &str) -> Option<bool> {
    let value = summary.split(marker).nth(1)?.split_whitespace().next()?;
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUMMARY: &str = "FIELD map layout=08000000 events=08000000 scripts=08000000 conns=08000000 music=0000 layoutId=0011 region=0 cave=0 weather=0 type=0 battle=0\nSAVE save1=02025734 route101=3 birch_lab=3 starter=1\nPARTY count=1 species=280 level=5 hp=20/20 checksum=1234\nSAVE_FLAGS pokemon_get=true rescued_birch=true hide_bag=true hide_zigzagoon=true hide_lab_birch=false route101_boy=false hide_may_1f=false hide_may_2f=false hide_may_ball=false";

    #[test]
    fn parses_emerald_debug_summary() {
        let snapshot = EduSnapshot::from_debug_summary(SUMMARY, 123);
        assert_eq!(snapshot.frame, 123);
        assert_eq!(snapshot.layout_id, Some(0x0011));
        assert_eq!(snapshot.party_count, Some(1));
        assert_eq!(snapshot.party_species, Some(280));
        assert_eq!(snapshot.starter_var, Some(1));
        assert_eq!(snapshot.pokemon_get, Some(true));
        assert_eq!(snapshot.rescued_birch, Some(true));
    }

    #[test]
    fn completes_matching_objectives_once() {
        let snapshot = EduSnapshot::from_debug_summary(SUMMARY, 456);
        let mut tracker = EduTracker::emerald_onboarding();
        let completed = tracker.update(&snapshot);
        let ids: Vec<_> = completed.iter().map(|objective| objective.id).collect();
        assert_eq!(ids, vec!["route101", "rescue_birch", "starter_acquired"]);
        assert!(tracker.update(&snapshot).is_empty());
        assert_eq!(tracker.objectives()[2].completed_at, Some(456));
    }

    #[test]
    fn leaves_unknown_or_missing_fields_pending() {
        let snapshot = EduSnapshot::from_debug_summary("FIELD map layoutId=000a\n", 7);
        let mut tracker = EduTracker::emerald_onboarding();
        let completed = tracker.update(&snapshot);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "littleroot");
        assert!(tracker.objectives()[4].completed_at.is_none());
    }
}
