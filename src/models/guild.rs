use serde::{Deserialize, Serialize};

/// Guild (Server) specific configuration
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GuildConfig {
    /// Channel ID where Ayumi (AI) is active
    pub ayumi_channel_id: Option<String>,
    /// Channel ID for Quiz events
    pub quiz_channel_id: Option<String>,
    /// Channel ID for welcome messages
    pub welcome_channel_id: Option<String>,
}
