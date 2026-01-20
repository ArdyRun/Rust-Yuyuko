use serde::{Deserialize, Serialize};
use rand::prelude::IndexedRandom;
use std::sync::OnceLock;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Novel {
    pub id: String,
    pub title: String,
    pub url: String,
    pub size: String,
    pub format: String,
}

static NOVELS: OnceLock<Vec<Novel>> = OnceLock::new();

/// Load novels from JSON file (Lazy loaded)
fn get_novels() -> &'static [Novel] {
    NOVELS.get_or_init(|| {
        match std::fs::read_to_string("src/data/novelList.json") {
            Ok(content) => {
                match serde_json::from_str::<Vec<Novel>>(&content) {
                    Ok(novels) => {
                        info!("Loaded {} novels", novels.len());
                        novels
                    },
                    Err(e) => {
                        error!("Failed to parse novelList.json: {:?}", e);
                        Vec::new()
                    }
                }
            },
            Err(e) => {
                error!("Failed to read novelList.json: {:?}", e);
                Vec::new()
            }
        }
    })
}

pub fn recommend_novels(count: usize) -> String {
    let novels = get_novels();
    if novels.is_empty() {
        return "Maaf, aku belum menemukan daftar novelnya... Sepertinya ada yang salah.".to_string();
    }

    let mut rng = rand::thread_rng();
    let selected: Vec<_> = novels.choose_multiple(&mut rng, count).collect();

    let mut response = "**Rekomendasi Novel untukmu:**\n\n".to_string();
    for (i, novel) in selected.iter().enumerate() {
        response.push_str(&format!("{}. [{}]({})\n   Format: {} | Size: {}\n\n", 
            i + 1, novel.title, novel.url, novel.format, novel.size));
    }
    
    response.push_str("Semoga suka ya! Jangan lupa baca ~ ❤️");
    response
}
