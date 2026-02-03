use once_cell::sync::Lazy;
use poise::serenity_prelude as serenity;
use std::collections::HashMap;
use tracing::error;

use crate::{Data, Error};
use std::env;

// --- Constants (Hardcoded from Go) ---
pub const KOTOBA_BOT_ID: serenity::UserId = serenity::UserId::new(251239170058616833);
// pub const QUIZ_SELECTOR_CHANNEL_ID: serenity::ChannelId = serenity::ChannelId::new(1392463011301691442); // Not strictly needed here but good for ref
// const QUIZ_CHANNEL_TTL: u64 = 24 * 60 * 60; // 24 hours, handle via scheduled task later if needed

// --- Data Structures ---

#[derive(Debug, Clone)]
pub struct QuizInfo {
    pub label: &'static str,
    pub description: &'static str,
    pub value: &'static str,
    pub role_id: serenity::RoleId,
    pub commands: &'static [&'static str],
    pub deck_names: &'static [&'static str],
    pub score_limits: &'static [&'static str],
    pub level: i32,
}

#[derive(Debug, Clone)]
pub struct QuizSession {
    pub user_id: serenity::UserId,
    pub quiz_id: String,
    pub thread_id: serenity::ChannelId, // The private channel ID
    pub started: bool,
    pub active_attempt: bool,
    pub progress: usize,
}

// --- Quiz Data Definitions ---

pub static QUIZZES: Lazy<HashMap<String, QuizInfo>> = Lazy::new(|| {
    let mut m = HashMap::new();

    m.insert(
        "hiragana_katakana".to_string(),
        QuizInfo {
            label: "Kanji Wakaran (漢字わからん)",
            level: 0,
            description: "Hiragana + Katakana Quiz",
            value: "hiragana_katakana",
            role_id: serenity::RoleId::new(1392065087216291891),
            commands: &[
                "k!quiz hiragana+katakana nd mmq=10 dauq=1 font=5 atl=16 color=#f173ff size=100",
            ],
            deck_names: &["Multiple Deck Quiz"],
            score_limits: &["10"],
        },
    );

    m.insert("Level_1".to_string(), QuizInfo {
        label: "Shoshinsha (初心者)",
        level: 1,
        description: "JPDB Beginner Level (1-300)",
        value: "Level_1",
        role_id: serenity::RoleId::new(1392065395984306246),
        commands: &["k!quiz jpdb300 20 hardcore nd mmq=10 dauq=1 font=5 atl=16 color=#f173ff size=100 effect=antiocr"],
        deck_names: &["jpdb300"],
        score_limits: &["20"],
    });

    m.insert("Level_2".to_string(), QuizInfo {
        label: "Gakushūsha (学習者)",
        level: 2,
        description: "JPDB Intermediate Level (300-1000)",
        value: "Level_2",
        role_id: serenity::RoleId::new(1392065532051591240),
        commands: &["k!quiz jpdb300to1k 25 hardcore nd mmq=10 dauq=1 font=5 atl=16 color=#f173ff size=100 effect=antiocr"],
        deck_names: &["jpdb300to1k"],
        score_limits: &["25"],
    });

    m.insert("Level_3".to_string(), QuizInfo {
        label: "Jōkyūsha (上級者)",
        level: 3,
        description: "JPDB Advance Level (100-3000)",
        value: "Level_3",
        role_id: serenity::RoleId::new(1392065673185857627),
        commands: &["k!quiz jpdb1k3k 30 hardcore nd mmq=10 dauq=1 font=5 atl=16 color=#f173ff size=100 effect=antiocr"],
        deck_names: &["jpdb1k3k"],
        score_limits: &["30"],
    });

    m.insert("Level_4".to_string(), QuizInfo {
        label: "Senpai (先輩)",
        level: 4,
        description: "JPDB 5000 + gn2",
        value: "Level_4",
        role_id: serenity::RoleId::new(1392066020235153408),
        commands: &[
            "k!quiz gn2 nd 20 mmq=4 atl=60",
            "k!quiz jpdb3k5k 40 hardcore nd mmq=10 dauq=1 font=5 atl=16 color=#f173ff size=100 effect=antiocr"
        ],
        deck_names: &["JLPT N2 Grammar Quiz", "jpdb3k5k"],
        score_limits: &["20", "40"],
    });

    m.insert("Level_5".to_string(), QuizInfo {
        label: "Tetsujin (鉄人)",
        level: 5,
        description: "JPDB 10K + gn1",
        value: "Level_5",
        role_id: serenity::RoleId::new(1392066105677189121),
        commands: &[
            "k!quiz gn1 nd 20 mmq=4 atl=60",
            "k!quiz jpdb5k10k 40 hardcore nd mmq=10 dauq=1 font=5 atl=16 color=#f173ff size=100 effect=antiocr"
        ],
        deck_names: &["JLPT N1 Grammar Quiz", "jpdb5k10k"],
        score_limits: &["20", "40"],
    });

    m.insert("Level_6".to_string(), QuizInfo {
        label: "Kotodama (言霊)",
        level: 6,
        description: "JPDB 20K + gn1",
        value: "Level_6",
        role_id: serenity::RoleId::new(1392066278335840376),
        commands: &[
            "k!quiz gn1 nd 20 mmq=4 atl=60",
            "k!quiz jpdb10k20k 45 hardcore nd mmq=10 dauq=1 font=5 atl=16 color=#f173ff size=100 effect=antiocr"
        ],
        deck_names: &["JLPT N1 Grammar Quiz", "jpdb10k20k"],
        score_limits: &["20", "45"],
    });

    m.insert("Level_7".to_string(), QuizInfo {
        label: "Koten Kami (古典神)",
        level: 7,
        description: "JPDB 30K",
        value: "Level_7",
        role_id: serenity::RoleId::new(1392066430467440742),
        commands: &["k!quiz jpdb20k30k+haado+cope+kunyomi1kfull+loli+Myouji+jpdefs+places_full 50 nd hardcore dauq=1 font=5 atl=16 mmq=9 color=#f173ff size=100 effect=antiocr"],
        deck_names: &["Multiple Deck Quiz"],
        score_limits: &["50"],
    });

    m
});

// --- Handlers ---

/// Handle "quiz_select" interaction
pub async fn handle_interaction(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    data: &Data,
) -> Result<(), Error> {
    if interaction.data.custom_id != "quiz_select" {
        return Ok(());
    }

    let user = &interaction.user;
    let guild_id = interaction.guild_id.ok_or("No guild ID")?;
    let quiz_id = match &interaction.data.kind {
        serenity::ComponentInteractionDataKind::StringSelect { values } => values.first(),
        _ => None,
    }
    .ok_or("No quiz selected")?;

    let quiz = match QUIZZES.get(quiz_id) {
        Some(q) => q,
        None => {
            let _ = interaction
                .create_response(
                    ctx,
                    serenity::CreateInteractionResponse::Message(
                        serenity::CreateInteractionResponseMessage::new()
                            .content("Quiz not found!")
                            .ephemeral(true),
                    ),
                )
                .await;
            return Ok(());
        }
    };

    // Check if user already has an active session
    if let Some(session) = data.role_rank_sessions.get(&user.id) {
        // Verify if channel still exists
        match ctx.http.get_channel(session.thread_id).await {
            Ok(_) => {
                let _ = interaction
                    .create_response(
                        ctx,
                        serenity::CreateInteractionResponse::Message(
                            serenity::CreateInteractionResponseMessage::new()
                                .content(
                                    "You already have an active quiz session! Finish it first.",
                                )
                                .ephemeral(true),
                        ),
                    )
                    .await;
                return Ok(());
            }
            Err(_) => {
                // Channel gone, remove session
                drop(session); // release lock
                data.role_rank_sessions.remove(&user.id);
            }
        }
    }

    // Create Private Channel
    let channel_name = format!(
        "quiz-{}-{}",
        user.name.to_lowercase(),
        quiz.label
            .split('(')
            .next()
            .unwrap_or("")
            .trim()
            .replace(' ', "-")
            .to_lowercase()
    );

    let permission_overwrites = vec![
        serenity::PermissionOverwrite {
            allow: serenity::Permissions::empty(),
            deny: serenity::Permissions::VIEW_CHANNEL,
            kind: serenity::PermissionOverwriteType::Role(serenity::RoleId::new(guild_id.get())),
        },
        serenity::PermissionOverwrite {
            allow: serenity::Permissions::VIEW_CHANNEL
                | serenity::Permissions::SEND_MESSAGES
                | serenity::Permissions::READ_MESSAGE_HISTORY,
            deny: serenity::Permissions::empty(),
            kind: serenity::PermissionOverwriteType::Member(user.id),
        },
        serenity::PermissionOverwrite {
            allow: serenity::Permissions::VIEW_CHANNEL
                | serenity::Permissions::SEND_MESSAGES
                | serenity::Permissions::READ_MESSAGE_HISTORY,
            deny: serenity::Permissions::empty(),
            kind: serenity::PermissionOverwriteType::Member(KOTOBA_BOT_ID),
        },
        serenity::PermissionOverwrite {
            allow: serenity::Permissions::VIEW_CHANNEL
                | serenity::Permissions::SEND_MESSAGES
                | serenity::Permissions::READ_MESSAGE_HISTORY,
            deny: serenity::Permissions::empty(),
            kind: serenity::PermissionOverwriteType::Member(ctx.cache.current_user().id),
        },
    ];

    // Get configured category ID or error
    let category_id = {
        if let Some(config) = data.guild_configs.get(&guild_id.to_string()) {
            config
                .quiz_category_id
                .as_ref()
                .and_then(|id| id.parse::<u64>().ok())
                .map(serenity::ChannelId::new)
        } else {
            None
        }
    };

    let category_id = match category_id {
        Some(id) => id,
        None => {
            let _ = interaction
                .create_response(
                    ctx,
                    serenity::CreateInteractionResponse::Message(
                        serenity::CreateInteractionResponseMessage::new()
                            .content(
                                "Quiz Category not configured! Ask admin to set it via /config.",
                            )
                            .ephemeral(true),
                    ),
                )
                .await;
            return Ok(());
        }
    };

    let builder = serenity::CreateChannel::new(channel_name)
        .kind(serenity::ChannelType::Text)
        .category(category_id)
        .permissions(permission_overwrites);

    let channel = match guild_id.create_channel(&ctx.http, builder).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create quiz channel: {:?}", e);
            let _ = interaction
                .create_response(
                    ctx,
                    serenity::CreateInteractionResponse::Message(
                        serenity::CreateInteractionResponseMessage::new()
                            .content("Failed to create private channel!")
                            .ephemeral(true),
                    ),
                )
                .await;
            return Ok(());
        }
    };

    // Store Session
    data.role_rank_sessions.insert(
        user.id,
        QuizSession {
            user_id: user.id,
            quiz_id: quiz_id.clone(),
            thread_id: channel.id,
            started: false,
            active_attempt: false,
            progress: 0,
        },
    );

    // Send Welcome Message
    let command_text = quiz.commands[0];
    let welcome_msg = format!(
        "Halo <@{}>! Untuk memulai quiz, copy dan paste command berikut:\n\n\
        **Command:**\n```\n{}\n```\n\n\
        **Cara bermain:**\n\
        1. Copy command di atas\n\
        2. Paste di channel ini\n\
        3. Jawab pertanyaan dari Kotoba Bot\n\
        4. Kamu akan mendapat role **{}** setelah menyelesaikan quiz!\n\
        5. Kamu bisa hapus channel ini secara manual dengan `a!del` (atau `/role_rank delete`)\n\n\
        Jangan lupa paste command langsung di channel ini ya!",
        user.id, command_text, quiz.label
    );

    let _ = channel.say(&ctx.http, welcome_msg).await;

    // Acknowledge Interaction
    let _ = interaction.create_response(ctx, serenity::CreateInteractionResponse::Message(
        serenity::CreateInteractionResponseMessage::new()
            .content(format!("Channel private **{}** telah dibuat untuk quiz **{}**. Silakan lanjut di sana!", channel.name, quiz.label))
            .ephemeral(true)
    )).await;

    Ok(())
}

/// Handle Message Events
pub async fn handle_message(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    data: &Data,
) -> Result<(), Error> {
    // 1. Handle User Starting Quiz
    if !msg.author.bot {
        if msg.content.starts_with("k!quiz") {
            // Check if this is an active session channel
            // We need to find if this channel belongs to ANY active session for THIS user
            if let Some(mut session) = data.role_rank_sessions.get_mut(&msg.author.id) {
                if session.thread_id == msg.channel_id {
                    let quiz = match QUIZZES.get(&session.quiz_id) {
                        Some(q) => q,
                        None => return Ok(()),
                    };

                    let expected_command = quiz.commands[session.progress];

                    if validate_command(&msg.content, expected_command) {
                        session.started = true;
                        session.active_attempt = true;
                        let _ = msg
                            .channel_id
                            .say(
                                &ctx.http,
                                "Command Valid! Menunggu hasil dari Kotoba Bot...",
                            )
                            .await;
                    } else {
                        session.active_attempt = false; // Invalidate previous attempt if any
                        let _ = msg.reply(&ctx.http, format!(
                            "**Command Tidak Sesuai**\nUntuk role ini, kamu wajib menggunakan command yang persis sama:\n```\n{}\n```\nJika kamu sedang menjalankan quiz, selesaikan dulu atau ketik `k!quiz stop` lalu paste commandnya lagi.", 
                            expected_command
                        )).await;
                    }
                }
            }
        }
        // Handle a!del (manual delete)
        else if msg.content.starts_with("a!del") {
            let channel = match msg.channel(&ctx.http).await {
                Ok(c) => c.guild().map(|gc| gc),
                Err(_) => None,
            };

            if let Some(gc) = channel {
                let guild_id = gc.guild_id.to_string();
                let category_id = if let Some(config) =
                    crate::utils::config::get_guild_config(data, &guild_id).await
                {
                    config
                        .quiz_category_id
                        .as_ref()
                        .and_then(|id| id.parse::<u64>().ok())
                        .map(serenity::ChannelId::new)
                } else {
                    None
                };

                if let Some(cat_id) = category_id {
                    if gc.parent_id == Some(cat_id) {
                        // Check if this is the configured selector channel
                        if let Some(config) =
                            crate::utils::config::get_guild_config(data, &guild_id).await
                        {
                            if let Some(selector_id) = &config.quiz_channel_id {
                                if gc.id.to_string() == *selector_id {
                                    let _ = msg.reply(&ctx.http, "Cannot delete main selector channel (Protected via Config).").await;
                                    return Ok(());
                                }
                            }
                        }

                        let _ = msg
                            .reply(&ctx.http, "Deleting channel in 3 seconds...")
                            .await;
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                        // Remove session first
                        // Use retain to remove any session pointing to this channel ID
                        data.role_rank_sessions.retain(|_, v| v.thread_id != gc.id);

                        if let Err(e) = gc.delete(&ctx.http).await {
                            error!("Failed to delete channel: {:?}", e);
                            let _ = msg
                                .reply(&ctx.http, format!("Failed to delete channel: {}", e))
                                .await;
                        }
                    } else {
                        let _ = msg
                            .reply(&ctx.http, "This command only works in quiz channels.")
                            .await;
                    }
                } // closing category_id
            }
        }
        // Handle a!clear <user_id> (Manual Role Reset)
        else if msg.content.starts_with("a!clear") {
            // 1. Permission Check
            let mut is_authorized = false;

            // Check Owner
            if let Ok(owner_id) = env::var("BOT_OWNER_ID") {
                if msg.author.id.to_string() == owner_id {
                    is_authorized = true;
                }
            }

            // Check Manage Guild
            if !is_authorized {
                if let Some(guild_id) = msg.guild_id {
                    if let Ok(member) = guild_id.member(&ctx.http, msg.author.id).await {
                        // Standard permission check
                        if let Some(guild) = guild_id.to_guild_cached(&ctx.cache) {
                            for role_id in &member.roles {
                                if let Some(role) = guild.roles.get(role_id) {
                                    if role
                                        .permissions
                                        .contains(serenity::Permissions::MANAGE_GUILD)
                                        || role
                                            .permissions
                                            .contains(serenity::Permissions::ADMINISTRATOR)
                                    {
                                        is_authorized = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !is_authorized {
                let _ = msg.reply(&ctx.http, "**Access Denied**: You need `MANAGE_GUILD` permissions or be the Bot Owner.").await;
                return Ok(());
            }

            // 2. Parse User ID
            let args: Vec<&str> = msg.content.split_whitespace().collect();
            if args.len() < 2 {
                let _ = msg
                    .reply(&ctx.http, "Usage: `a!clear <user_id>` (or mention)")
                    .await;
                return Ok(());
            }

            let target_str = args[1].trim_start_matches("<@").trim_end_matches(">");
            let target_id = match target_str.parse::<u64>() {
                Ok(id) => serenity::UserId::new(id),
                Err(_) => {
                    let _ = msg.reply(&ctx.http, "Invalid User ID format.").await;
                    return Ok(());
                }
            };

            // 3. Remove Roles
            if let Some(guild_id) = msg.guild_id {
                match guild_id.member(&ctx.http, target_id).await {
                    Ok(member) => {
                        let mut removed_count = 0;
                        for quiz in QUIZZES.values() {
                            if member.roles.contains(&quiz.role_id) {
                                if let Err(e) = member.remove_role(&ctx.http, quiz.role_id).await {
                                    error!(
                                        "Failed to remove role {} for user {}: {:?}",
                                        quiz.role_id, target_id, e
                                    );
                                } else {
                                    removed_count += 1;
                                }
                            }
                        }

                        let _ = msg
                            .reply(
                                &ctx.http,
                                format!(
                                    "**Reset Complete**: Removed {} quiz roles from <@{}>.",
                                    removed_count, target_id
                                ),
                            )
                            .await;
                    }
                    Err(_) => {
                        let _ = msg.reply(&ctx.http, "User not found in this server.").await;
                    }
                }
            }
        }
        return Ok(());
    }

    // 2. Handle Kotoba Bot Messages
    if msg.author.id == KOTOBA_BOT_ID {
        handle_kotoba_message(ctx, msg, data).await?;
    }

    Ok(())
}

async fn handle_kotoba_message(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    data: &Data,
) -> Result<(), Error> {
    if msg.embeds.is_empty() {
        return Ok(());
    }

    for embed in &msg.embeds {
        // Check for "Congratulations!" in Title OR Description
        let mut is_congrats = false;

        if let Some(title) = &embed.title {
            if title.contains("Congratulations!") {
                is_congrats = true;
            }
        }
        if !is_congrats {
            if let Some(desc) = &embed.description {
                if desc.contains("Congratulations!") {
                    is_congrats = true;
                }
            }
        }

        if !is_congrats {
            continue;
        }

        // Identify User & Session
        // We have to iterate sessions to find one that matches channel_id AND is started
        let user_id;
        {
            // Scope to release Ref
            let session_entry = data.role_rank_sessions.iter().find(|entry| {
                entry.value().thread_id == msg.channel_id
                    && entry.value().started
                    && entry.value().active_attempt
            });

            if let Some(entry) = session_entry {
                user_id = *entry.key();
            } else {
                return Ok(());
            }
        }

        let mut session = if let Some(s) = data.role_rank_sessions.get_mut(&user_id) {
            s
        } else {
            return Ok(());
        };
        let quiz = match QUIZZES.get(&session.quiz_id) {
            Some(q) => q,
            None => return Ok(()),
        };

        if session.progress >= quiz.commands.len() {
            return Ok(());
        }

        // --- Validate Embed ---
        let expected_deck = quiz.deck_names[session.progress].to_lowercase();
        let expected_score = quiz.score_limits[session.progress].to_lowercase();

        // 1. Check if Title indicates Score Limit Reached (This overrides Deck Name check)
        // Title format: "The score limit of <SCORE> was reached by <USER>. Congratulations!"
        let title = embed.title.clone().unwrap_or_default();
        let mut score_limit_reached = false;

        if title.contains("The score limit of") && title.contains("was reached") {
            // Extract score from title
            let parts: Vec<&str> = title.split_whitespace().collect();
            for (i, word) in parts.iter().enumerate() {
                if *word == "of" && i + 1 < parts.len() {
                    let s = parts[i + 1];
                    if s == expected_score {
                        score_limit_reached = true;
                    }
                }
            }
        }

        if score_limit_reached {
            // Success! Title confirms score limit was reached.
            // We skip deck name check because the title is overwritten.
        } else {
            // Fallback to standard check (Deck Name + Score in fields/desc)
            // 1. Check Deck Name (from Title)
            let title_deck = title.trim_end_matches(" Ended").to_lowercase();

            // 2. Check Score (from Fields or Description)
            let mut actual_score = String::new();

            for field in &embed.fields {
                if field.name.to_lowercase().contains("score limit") {
                    actual_score = field.value.to_lowercase();
                    break;
                }
            }

            if actual_score.is_empty() {
                if let Some(desc) = &embed.description {
                    let lower_desc = desc.to_lowercase();
                    if let Some(idx) = lower_desc.find("score limit of ") {
                        let rest = &lower_desc[idx + 15..];
                        actual_score = rest.split_whitespace().next().unwrap_or("").to_string();
                    }
                }
            }

            // Clean score (take first part if includes spaces/text)
            actual_score = actual_score
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();

            if !title_deck.contains(&expected_deck) || actual_score != expected_score {
                // Double check if strict deck check is too strict or if title mismatch provided
                if !score_limit_reached {
                    let _ = msg.channel_id.say(&ctx.http, 
                        format!("⚠️ **Validasi Gagal**\nDeck atau Score tidak sesuai.\nExpected Deck: {}\nExpected Score: {}\nDetected Deck: {}\nDetected Score: {}", 
                        expected_deck, expected_score, title_deck, actual_score)
                    ).await;
                    return Ok(());
                }
            }
        }

        // --- Success ---

        // Check if there are more stages
        if session.progress + 1 < quiz.commands.len() {
            session.progress += 1;
            let next_cmd = quiz.commands[session.progress];

            let _ = msg
                .channel_id
                .say(
                    &ctx.http,
                    format!(
                        "Stage selesai! Lanjut ke tahap berikutnya:\n```{}```",
                        next_cmd
                    ),
                )
                .await;
        } else {
            // All stages complete!
            // Assign Role
            session.started = false; // Stop tracking
            session.active_attempt = false;

            let guild_id = msg.guild_id.unwrap();
            let member = guild_id.member(&ctx.http, user_id).await?;

            // Check Current Roles (Prevent Downgrade/Duplicate)
            // Implementation simplified: just add role and remove old ones if we implement exclusive logic later.
            // For now, based on Go code:

            // Go code logic:
            // 1. Get current level from owned roles.
            // 2. If already same level -> Done.
            // 3. If higher level -> "Downgrade not allowed".
            // 4. Else -> Remove old role, Add new role.

            let current_level = get_current_quiz_level(&member);

            if current_level == quiz.level {
                let _ = msg.channel_id.say(&ctx.http, format!("Kamu sudah memiliki role **{}**. Tidak ada perubahan.\nChannel akan dihapus dalam 30 detik.", quiz.label)).await;
            } else if current_level > quiz.level {
                let _ = msg.channel_id.say(&ctx.http, "Kamu sudah memiliki role tier lebih tinggi. Tidak bisa downgrade.\nChannel akan dihapus dalam 30 detik.").await;
            } else {
                // Remove old role (if any)
                if current_level >= 0 {
                    // find old role id
                    for q in QUIZZES.values() {
                        if q.level == current_level {
                            let _ = member.remove_role(&ctx.http, q.role_id).await;
                        }
                    }
                }

                // Add new role
                if let Err(e) = member.add_role(&ctx.http, quiz.role_id).await {
                    error!("Failed to add role: {:?}", e);
                    let _ = msg
                        .channel_id
                        .say(&ctx.http, "Gagal menambahkan role. Hubungi admin.")
                        .await;
                } else {
                    let _ = msg.channel_id.say(&ctx.http, format!(
                        "**SELAMAT**! Kamu sekarang mendapatkan role **{}**.\nChannel ini akan dihapus dalam 30 detik.", 
                        quiz.label
                    )).await;

                    // Announcement to public channel
                    if let Some(cfg) =
                        crate::utils::config::get_guild_config(data, &guild_id.to_string()).await
                    {
                        if let Some(annu_id) = &cfg.role_rank_announcement_channel_id {
                            if let Ok(target_channel) = annu_id.parse::<serenity::ChannelId>() {
                                let _ = target_channel.say(&ctx.http, format!(
                                    "Selamat kepada <@{}> yang telah berhasil mendapatkan role **{}**!",
                                    member.user.id, quiz.label
                                )).await;
                            }
                        }
                    }
                }
            }

            // Cleanup
            let http = ctx.http.clone();
            let channel_id = msg.channel_id;
            let u_id = user_id;
            let sessions = data.role_rank_sessions.clone();

            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                let _ = channel_id.delete(&http).await;
                sessions.remove(&u_id);
            });
        }
    }

    Ok(())
}

fn get_current_quiz_level(member: &serenity::Member) -> i32 {
    for role_id in &member.roles {
        for quiz in QUIZZES.values() {
            if role_id == &quiz.role_id {
                return quiz.level;
            }
        }
    }
    -1
}

fn validate_command(user_input: &str, expected: &str) -> bool {
    let u = user_input.trim();
    let e = expected.trim();

    // Simple equality check for now (Strict Mode)
    // We can make this smarter later if needed (e.g. order of params)
    u == e
}
