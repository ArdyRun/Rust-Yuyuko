use poise::serenity_prelude as serenity;
use tracing::{error, debug};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use lru::LruCache;
use std::num::NonZeroUsize;
use once_cell::sync::Lazy;
use chrono::{DateTime, Utc};

use crate::features::novel_recommender::smart_novel_search;
use crate::features::custom_prompt::get_user_custom_prompt;
use crate::Data;
use crate::models::guild::GuildConfig;
use crate::api::llm::{completion_openrouter, completion_gemini_vision, generate_image, ChatMessage};
use crate::utils::ayumi_prompt::AYUMI_SYSTEM_PROMPT;

// ============ User Context ============

/// User data with context for personalized responses
#[derive(Debug, Clone)]
pub struct UserData {
    #[allow(dead_code)]
    pub user_id: u64,
    #[allow(dead_code)]
    pub username: String,
    #[allow(dead_code)]
    pub display_name: String,
    pub nickname: Option<String>,
    pub best_name: String,
    pub interaction_count: u32,
    pub last_interaction: DateTime<Utc>,
    #[allow(dead_code)]
    pub conversation_history: Vec<ChatMessage>,
}

impl UserData {
    pub fn new(user_id: u64, username: &str, display_name: &str, nickname: Option<&str>) -> Self {
        let best_name = nickname.unwrap_or(display_name).to_string();
        Self {
            user_id,
            username: username.to_string(),
            display_name: display_name.to_string(),
            nickname: nickname.map(|s| s.to_string()),
            best_name,
            interaction_count: 1,
            last_interaction: Utc::now(),
            conversation_history: Vec::new(),
        }
    }
}

// Global caches
type HistoryCache = LruCache<u64, Vec<ChatMessage>>;
type UserCache = HashMap<u64, UserData>;

static CONVERSATION_HISTORY: Lazy<Arc<Mutex<HistoryCache>>> = Lazy::new(|| {
    Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap())))
});

static USER_DATA: Lazy<Arc<Mutex<UserCache>>> = Lazy::new(|| {
    Arc::new(Mutex::new(HashMap::new()))
});

// ============ Detection Functions ============

fn detect_image_generation(text: &str) -> bool {
    let keywords = [
        "buatkan gambar", "generate gambar", "buat gambar", "gambarkan",
        "draw", "create image", "bikin gambar", "lukis", "sketch",
        "ilustrasi", "visualisasi", "make an image", "buatkan ilustrasi",
        "create illustration", "gambar anime", "anime art", "pixel art", "artwork"
    ];
    let lower = text.to_lowercase();
    keywords.iter().any(|k| lower.contains(k))
}

fn detect_avatar_question(text: &str) -> bool {
    let keywords = [
        "foto profil", "avatar", "profile picture", "pp", "foto pp",
        "gambar profil", "foto saya", "avatar saya", "pp saya",
        "lihat foto", "foto gue", "avatar gue", "pp gue", "pfp"
    ];
    let lower = text.to_lowercase();
    keywords.iter().any(|k| lower.contains(k))
}

fn detect_novel_request(text: &str) -> bool {
    let keywords = [
        "novel", "light novel", "cari novel", "rekomendasi novel",
        "download novel", "unduh novel", "novel saran", "novel untuk",
        "novel pemula", "novel jlpt", "novel n5", "novel n4", "novel n3",
        "novel n2", "novel n1", "novel romance", "novel isekai"
    ];
    let lower = text.to_lowercase();
    keywords.iter().any(|k| lower.contains(k))
}

// ============ Smart Message Chunking ============

/// Split message by lines to avoid cutting words
fn smart_chunk_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    
    for line in text.lines() {
        // If single line is too long, split it
        if line.len() > max_len {
            // First, push current chunk if not empty
            if !current_chunk.is_empty() {
                chunks.push(current_chunk);
                current_chunk = String::new();
            }
            
            // Split long line by words
            let mut word_chunk = String::new();
            for word in line.split_whitespace() {
                if word_chunk.len() + word.len() + 1 > max_len {
                    if !word_chunk.is_empty() {
                        chunks.push(word_chunk);
                    }
                    word_chunk = word.to_string();
                } else {
                    if !word_chunk.is_empty() {
                        word_chunk.push(' ');
                    }
                    word_chunk.push_str(word);
                }
            }
            if !word_chunk.is_empty() {
                current_chunk = word_chunk;
            }
        } else if current_chunk.len() + line.len() + 1 > max_len {
            // Push current chunk and start new one
            if !current_chunk.is_empty() {
                chunks.push(current_chunk);
            }
            current_chunk = line.to_string();
        } else {
            // Add line to current chunk
            if !current_chunk.is_empty() {
                current_chunk.push('\n');
            }
            current_chunk.push_str(line);
        }
    }
    
    // Don't forget last chunk
    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }
    
    chunks
}

// ============ Main Handler ============

pub async fn handle_message(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    data: &Data,
) -> Result<(), anyhow::Error> {
    if msg.author.bot {
        return Ok(());
    }

    let guild_id = match msg.guild_id {
        Some(gid) => gid.to_string(),
        None => return Ok(()),
    };

    // Get guild config
    let config = if let Some(cached) = data.guild_configs.get(&guild_id) {
        cached.clone()
    } else {
        match data.firebase.get_document("guilds", &guild_id).await {
            Ok(Some(doc)) => {
                let cfg = serde_json::from_value::<GuildConfig>(doc).unwrap_or_default();
                data.guild_configs.insert(guild_id.clone(), cfg.clone());
                cfg
            },
            Ok(None) => return Ok(()),
            Err(e) => {
                error!("Failed to fetch guild config for {}: {:?}", guild_id, e);
                return Ok(());
            }
        }
    };

    let ayumi_channel = match config.ayumi_channel_id {
        Some(id) => id,
        None => return Ok(()),
    };

    if msg.channel_id.to_string() != ayumi_channel {
        return Ok(());
    }

    let _typing = msg.channel_id.start_typing(&ctx.http);

    // Get or create user data
    let user_id = msg.author.id.get();
    let nickname = msg.member.as_ref().and_then(|m| m.nick.as_deref());
    let display_name = msg.author.global_name.as_deref().unwrap_or(&msg.author.name);
    
    let (user_name, interaction_count) = {
        let mut users = USER_DATA.lock().await;
        let user_data = users.entry(user_id).or_insert_with(|| {
            UserData::new(user_id, &msg.author.name, display_name, nickname)
        });
        user_data.interaction_count += 1;
        user_data.last_interaction = Utc::now();
        if nickname.is_some() {
            user_data.nickname = nickname.map(|s| s.to_string());
            user_data.best_name = nickname.unwrap().to_string();
        }
        (user_data.best_name.clone(), user_data.interaction_count)
    };

    // Get conversation history
    let history_clone = {
        let mut cache = CONVERSATION_HISTORY.lock().await;
        cache.get(&user_id).cloned().unwrap_or_default()
    };

    let mut messages = history_clone;
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: msg.content.clone(),
    });

    // Check for image attachment
    let attachment = msg.attachments.iter().find(|a| {
        a.content_type.as_ref().map_or(false, |ct| ct.starts_with("image/"))
    });

    let response: String;

    if let Some(att) = attachment {
        debug!("Processing image attachment for user {}", user_name);
        
        let image_data = match att.download().await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to download attachment: {:?}", e);
                msg.reply(ctx, "Gagal mengunduh gambar...").await?;
                return Ok(());
            }
        };

        let prompt = if msg.content.trim().is_empty() {
            "Deskripsikan gambar ini dengan gaya bahasa Ayumi."
        } else {
            &msg.content
        };

        let mime_type = att.content_type.as_deref().unwrap_or("image/jpeg");
        
        response = match completion_gemini_vision(data, prompt, &image_data, mime_type).await {
            Ok(res) => res,
            Err(e) => {
                error!("Ayumi Gemini Vision error: {:?}", e);
                "Maaf, mataku agak buram... Gak bisa liat gambarnya jelas.".to_string()
            }
        };
    } else if detect_image_generation(&msg.content) {
        debug!("Processing image generation for user {}", user_name);
        
        let generating_msg = msg.reply(ctx, format!("{}, Ayumi lagi bikin gambar sesuai request kamu nih! Tunggu sebentar ya...", user_name)).await?;
        
        match generate_image(data, &msg.content).await {
            Ok(result) => {
                let _ = generating_msg.delete(ctx).await;
                let extension = if result.mime_type.contains("png") { "png" } else { "jpg" };
                let filename = format!("ayumi_generated_{}.{}", chrono::Utc::now().timestamp(), extension);
                
                let attachment = serenity::CreateAttachment::bytes(result.image_data, filename);
                let reply_content = format!("{}, nih gambar yang Ayumi buatin! Gimana, sesuai ekspektasi gak?", user_name);
                
                msg.channel_id.send_message(ctx, serenity::CreateMessage::new()
                    .content(&reply_content)
                    .add_file(attachment)
                ).await?;
                
                response = reply_content;
            }
            Err(e) => {
                error!("Image generation failed: {:?}", e);
                let _ = generating_msg.delete(ctx).await;
                response = format!("{}, maaf nih Ayumi lagi gabisa bikin gambar. Coba lagi nanti ya", user_name);
                msg.reply(ctx, &response).await?;
            }
        };
        
        // Update history and return
        {
            let mut cache = CONVERSATION_HISTORY.lock().await;
            messages.push(ChatMessage { role: "assistant".to_string(), content: response.clone() });
            if messages.len() > 20 {
                messages = messages.iter().rev().take(20).rev().cloned().collect();
            }
            cache.put(user_id, messages);
        }
        return Ok(());
        
    } else if detect_avatar_question(&msg.content) {
        debug!("Processing avatar analysis for user {}", user_name);
        
        let avatar_url = msg.author.avatar_url().unwrap_or_else(|| msg.author.default_avatar_url());
        
        let avatar_response = match data.http_client.get(&avatar_url).send().await {
            Ok(res) => res,
            Err(e) => {
                error!("Failed to fetch avatar: {:?}", e);
                msg.reply(ctx, "Ayumi gak bisa liat foto profil kamu...").await?;
                return Ok(());
            }
        };
        
        let avatar_data = match avatar_response.bytes().await {
            Ok(bytes) => bytes.to_vec(),
            Err(e) => {
                error!("Failed to read avatar bytes: {:?}", e);
                msg.reply(ctx, "Ayumi gak bisa liat foto profil kamu...").await?;
                return Ok(());
            }
        };
        
        let prompt = format!(
            "Kamu adalah Ayumi. User {} (sudah {} kali ngobrol sama kamu) minta lihat foto profil mereka. Pertanyaan: \"{}\". Analisis dan komentar foto profil ini dengan fun tapi sopan.",
            user_name, interaction_count, msg.content
        );
        
        response = match completion_gemini_vision(data, &prompt, &avatar_data, "image/png").await {
            Ok(res) => res,
            Err(e) => {
                error!("Avatar analysis error: {:?}", e);
                format!("{}, Ayumi pengen lihat foto profil kamu tapi lagi error nih!", user_name)
            }
        };
        
    } else if detect_novel_request(&msg.content) {
        debug!("Processing smart novel search for user {}", user_name);
        response = smart_novel_search(data, &msg.content).await;
    } else {
        debug!("Processing text chat for user {} (interaction #{})", user_name, interaction_count);
        
        // Build context with user info
        let user_context = format!(
            "User ini namanya {}. Sudah {} kali berinteraksi dengan Ayumi.",
            user_name, interaction_count
        );
        
        let system_prompt = get_user_custom_prompt(user_id)
            .unwrap_or_else(|| AYUMI_SYSTEM_PROMPT.to_string());
        
        let full_prompt = format!("{}\n\n{}", system_prompt, user_context);
        
        response = match completion_openrouter(data, &full_prompt, messages.clone()).await {
            Ok(res) => res,
            Err(e) => {
                error!("Ayumi LLM error: {:?}", e);
                "Maaf, Ayumi lagi pusing... Coba lagi nanti ya.".to_string()
            }
        };
    };

    // Send reply with smart chunking
    let chunks = smart_chunk_message(&response, 1950);
    for (i, chunk) in chunks.iter().enumerate() {
        if i == 0 {
            let content = if chunks.len() > 1 {
                format!("{}\n\n*Lanjut di pesan berikutnya...*", chunk)
            } else {
                chunk.to_string()
            };
            msg.reply(ctx, &content).await?;
        } else if i == chunks.len() - 1 {
            msg.channel_id.say(&ctx.http, chunk).await?;
        } else {
            msg.channel_id.say(&ctx.http, format!("{}\n\n*Lanjut...*", chunk)).await?;
        }
    }

    // Update history
    {
        let mut cache = CONVERSATION_HISTORY.lock().await;
        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: response,
        });
        
        if messages.len() > 20 {
            messages = messages.iter().rev().take(20).rev().cloned().collect();
        }
        
        cache.put(user_id, messages);
    }

    Ok(())
}
