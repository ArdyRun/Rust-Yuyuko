# Code Audit: Yuyuko Rust Bot

**Date:** 2025-01-26
**Scope:** Performance, Reliability, and Security Review
**Version:** yuyuko-rs v0.1.0

## 🔴 Critical Issues

### 1. Data Race in Immersion Stats Update
- **Description:** `commands/immersion.rs` reads the user's current stats, increments them in memory, and writes the entire stats object back to Firestore using `set_document`.
- **Risk:** High. If a user logs two activities simultaneously (e.g. from two different devices or rapid commands), the second write will overwrite the first one's increment. This leads to data loss (lost immersion points).
- **Location:** `src/commands/immersion.rs` (Lines ~324-370) and `src/api/firebase.rs` (`set_document`).
- **Mitigation:** Use Firestore `transform` operations with `FieldValue.increment()` to atomically update counters, instead of reading and replacing the whole object.

### 2. Scalability Risk in Streak Calculation
- **Description:** `immersion.rs` fetches **all** of a user's immersion logs to calculate the current streak every time they log an activity.
- **Risk:** High. As users accumulate history (e.g. 1000+ logs), this operation becomes O(N). It will eventually cause timeouts, excessive memory usage, and massive bandwidth costs/quota usage.
- **Location:** `src/commands/immersion.rs` (Lines ~394-427).
- **Mitigation:** Store the `current_streak` and `last_log_date` directly on the User document. Update these fields incrementally when a new log is added. Only fetch logs for history repair/audit.

### 3. Scalability Risk in Leaderboard
- **Description:** `leaderboard.rs` fetches **all** users from the database to generate the leaderboard.
- **Risk:** High. O(N) where N is the total user base. This will become too slow to run as the server grows.
- **Location:** `src/commands/leaderboard.rs` (Line ~99 `get_all_users`).
- **Mitigation:** Use Firestore queries with `orderBy` and `limit` to fetch only the top users. Maintain separate counters/indexes if necessary for complex sorts.

---

## 🟠 Medium-Risk Issues

### 1. Inefficient Role Rank Session Lookup
- **Description:** `features/role_rank.rs` iterates over all active sessions for *every* message sent by Kotoba Bot to find the matching channel.
- **Risk:** Medium. O(S) per message where S is active sessions. If many users are taking quizzes, this slows down message processing for the bot.
- **Location:** `src/features/role_rank.rs` (Lines ~460-470).
- **Mitigation:** Maintain a secondary mapping of `ChannelId -> UserId` or `ChannelId -> Session` to allow O(1) lookup.

### 2. Hardcoded Configuration
- **Description:** Bot ID (`KOTOBA_BOT_ID`) and Role IDs are hardcoded in the source code.
- **Risk:** Medium. The bot is brittle and cannot be deployed to other servers or updated without recompilation.
- **Location:** `src/features/role_rank.rs`.
- **Mitigation:** Move these IDs to `GuildConfig` or a configuration file.

### 3. Thundering Herd in Token Generation
- **Description:** `api/firebase.rs` does not double-check the token cache after acquiring the write lock. Concurrent requests when the token is expired will all trigger a new token generation request.
- **Risk:** Medium. Wastes Google API quota and CPU.
- **Location:** `src/api/firebase.rs` (`get_access_token`).
- **Mitigation:** Implement double-checked locking: check the cache again after acquiring the write lock before generating a new token.

---

## 🟡 Minor Improvements

- **Unimplemented Leaderboard Features:** The leaderboard command offers Weekly/Monthly options but returns All-Time stats for them (`src/commands/leaderboard.rs`). This is misleading to users.
- **Potential Panics:** Usage of `unwrap()` in `role_rank.rs` (e.g., parsing scores or titles) could cause the thread to panic if the format changes unexpectedly.
- **Blocking HTML Parsing:** `immersion.rs` parses HTML for page titles on the async thread. For very large pages, this could block the runtime slightly.

---

## ⚙️ Performance Observations

- **Async Runtime:** The project correctly uses `tokio` and `poise`.
- **HTTP Client:** `reqwest::Client` is correctly shared via `Data`.
- **Database:** `DashMap` is used for caching, which is good for concurrent read access. However, the `immersion_logs` fetch pattern defeats the purpose of a fast database.

---

## 🔐 Security Observations

- **Secret Management:** The Firebase private key is loaded from a file (`firebase-key.json`). In containerized environments, passing this via an environment variable is often more secure and flexible.
- **Permissions:** The bot uses `MESSAGE_CONTENT` intent, which is privileged. Ensure this is strictly necessary (it is, for the quiz feature).
- **Input Validation:** Command inputs (`amount`, `date`) are well-validated.

---

## ✅ Positive Practices Detected

- **Architecture:** Clean separation of concerns (API, Commands, Features, Models).
- **Type Safety:** Strong usage of Rust types to model Firestore documents.
- **Concurrency:** Usage of `DashMap` and `Arc` for shared state is correct.
- **Logging:** `tracing` is used effectively for observability.
