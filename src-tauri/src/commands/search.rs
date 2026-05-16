//! Command-palette search. The renderer feeds it a query plus a flat
//! list of candidate items it already has cached; we just rank by a
//! simple substring + start-of-token match. Real cross-corpus search
//! lives in a follow-up.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Deserialize, Type)]
pub struct CommandPaletteItem {
    pub id: String,
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Serialize, Type)]
pub struct CommandPaletteResult {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub score: i32,
}

#[tauri::command]
#[specta::specta]
pub fn search_command_palette(
    query: String,
    items: Vec<CommandPaletteItem>,
) -> Vec<CommandPaletteResult> {
    let q = query.to_lowercase();
    if q.is_empty() {
        return items
            .into_iter()
            .take(50)
            .map(|i| CommandPaletteResult {
                id: i.id,
                label: i.label,
                kind: i.kind,
                score: 0,
            })
            .collect();
    }
    let mut scored: Vec<CommandPaletteResult> = items
        .into_iter()
        .filter_map(|i| {
            let l = i.label.to_lowercase();
            let score = if l == q {
                1000
            } else if l.starts_with(&q) {
                500
            } else if l.contains(&q) {
                250
            } else {
                return None;
            };
            Some(CommandPaletteResult {
                id: i.id,
                label: i.label,
                kind: i.kind,
                score,
            })
        })
        .collect();
    scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
    scored.truncate(50);
    scored
}
