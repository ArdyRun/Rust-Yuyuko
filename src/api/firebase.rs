// Firebase Firestore REST API client
// Using service account JWT authentication

use anyhow::{anyhow, Result};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error};

/// Firebase service account credentials
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceAccount {
    pub project_id: String,
    pub private_key: String,
    pub client_email: String,
}

/// JWT claims for Google OAuth2
#[derive(Debug, Serialize)]
struct Claims {
    iss: String,
    sub: String,
    aud: String,
    iat: u64,
    exp: u64,
    scope: String,
}

/// Cached access token
struct CachedToken {
    token: String,
    expires_at: u64,
}

/// Firebase REST API client
pub struct FirebaseClient {
    client: Client,
    service_account: ServiceAccount,
    token_cache: Arc<RwLock<Option<CachedToken>>>,
}

impl FirebaseClient {
    /// Create a new Firebase client from service account JSON file
    pub fn from_file(client: Client, path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let service_account: ServiceAccount = serde_json::from_str(&content)?;

        Ok(Self {
            client,
            service_account,
            token_cache: Arc::new(RwLock::new(None)),
        })
    }

    /// Get access token (with caching)
    async fn get_access_token(&self) -> Result<String> {
        // Check cache first
        {
            let cache = self.token_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                // Return cached token if still valid (with 60s buffer)
                if cached.expires_at > now + 60 {
                    return Ok(cached.token.clone());
                }
            }
        }

        // Generate new token
        let token = self.generate_access_token().await?;

        // Cache it
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        {
            let mut cache = self.token_cache.write().await;
            *cache = Some(CachedToken {
                token: token.clone(),
                expires_at: now + 3600, // 1 hour
            });
        }

        Ok(token)
    }

    /// Generate a new access token using JWT
    async fn generate_access_token(&self) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            iss: self.service_account.client_email.clone(),
            sub: self.service_account.client_email.clone(),
            aud: "https://oauth2.googleapis.com/token".to_string(),
            iat: now,
            exp: now + 3600,
            scope: "https://www.googleapis.com/auth/datastore".to_string(),
        };

        // Encode JWT
        let key = EncodingKey::from_rsa_pem(self.service_account.private_key.as_bytes())?;
        let jwt = encode(&Header::new(Algorithm::RS256), &claims, &key)?;

        // Exchange JWT for access token
        let response = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &jwt),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await?;
            error!("Failed to get access token: {}", body);
            return Err(anyhow!("Failed to get access token"));
        }

        let data: Value = response.json().await?;
        let token = data["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No access_token in response"))?;

        Ok(token.to_string())
    }

    /// Base URL for Firestore REST API
    fn base_url(&self) -> String {
        format!(
            "https://firestore.googleapis.com/v1/projects/{}/databases/(default)/documents",
            self.service_account.project_id
        )
    }

    /// Get a document by path
    pub async fn get_document(&self, collection: &str, doc_id: &str) -> Result<Option<Value>> {
        let token = self.get_access_token().await?;
        let url = format!("{}/{}/{}", self.base_url(), collection, doc_id);

        let response = self.client.get(&url).bearer_auth(&token).send().await?;

        if response.status() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            debug!("Firebase error: {}", body);
            return Err(anyhow!("Firebase error: {}", status));
        }

        let doc: Value = response.json().await?;
        Ok(Some(from_firestore_document(&doc)))
    }

    /// Set/update a document (merge)
    pub async fn set_document(&self, collection: &str, doc_id: &str, data: &Value) -> Result<()> {
        let token = self.get_access_token().await?;

        // Build updateMask from top-level field names
        let field_paths: String = data
            .as_object()
            .map(|obj| {
                obj.keys()
                    .map(|k| format!("updateMask.fieldPaths={}", k))
                    .collect::<Vec<_>>()
                    .join("&")
            })
            .unwrap_or_default();

        let url = format!(
            "{}/{}/{}?{}",
            self.base_url(),
            collection,
            doc_id,
            field_paths
        );

        let firestore_doc = to_firestore_document(data);

        let response = self
            .client
            .patch(&url)
            .bearer_auth(&token)
            .json(&firestore_doc)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            debug!("Firebase error: {}", body);
            return Err(anyhow!("Firebase error: {}", status));
        }

        Ok(())
    }

    /// Add a document to a subcollection
    pub async fn add_to_subcollection(
        &self,
        collection: &str,
        doc_id: &str,
        subcollection: &str,
        data: &Value,
    ) -> Result<String> {
        let token = self.get_access_token().await?;
        let url = format!(
            "{}/{}/{}/{}",
            self.base_url(),
            collection,
            doc_id,
            subcollection
        );

        let firestore_doc = to_firestore_document(data);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&firestore_doc)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            debug!("Firebase error: {}", body);
            return Err(anyhow!("Firebase error: {}", status));
        }

        let result: Value = response.json().await?;
        let name = result["name"].as_str().unwrap_or("");
        let id = name.split('/').last().unwrap_or("");
        Ok(id.to_string())
    }

    /// Query a subcollection - returns just the data
    pub async fn query_subcollection(
        &self,
        collection: &str,
        doc_id: &str,
        subcollection: &str,
    ) -> Result<Vec<Value>> {
        let docs = self
            .query_subcollection_with_ids(collection, doc_id, subcollection)
            .await?;
        Ok(docs.into_iter().map(|(_, v)| v).collect())
    }

    /// Query a subcollection with filters - returns (id, data) tuples
    /// Handles pagination to fetch ALL documents
    pub async fn query_subcollection_with_ids(
        &self,
        collection: &str,
        doc_id: &str,
        subcollection: &str,
    ) -> Result<Vec<(String, Value)>> {
        let token = self.get_access_token().await?;
        let base_url = format!(
            "{}/{}/{}/{}",
            self.base_url(),
            collection,
            doc_id,
            subcollection
        );

        let mut all_docs = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut url = format!("{}?pageSize=300", base_url);
            if let Some(ref t) = page_token {
                url.push_str(&format!("&pageToken={}", t));
            }

            let response = self.client.get(&url).bearer_auth(&token).send().await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await?;
                debug!("Firebase error: {}", body);
                return Err(anyhow!("Firebase error: {}", status));
            }

            let result: Value = response.json().await?;

            if let Some(arr) = result["documents"].as_array() {
                for doc in arr {
                    if let Some(id) = doc["name"]
                        .as_str()
                        .and_then(|name| name.split('/').last())
                        .map(|s| s.to_string())
                    {
                        let data = from_firestore_document(doc);
                        all_docs.push((id, data));
                    }
                }
            }

            // Check for next page
            match result.get("nextPageToken") {
                Some(t) => {
                    if let Some(t_str) = t.as_str() {
                        page_token = Some(t_str.to_string());
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }

        Ok(all_docs)
    }

    /// Delete a document
    pub async fn delete_document(&self, collection: &str, doc_id: &str) -> Result<()> {
        let token = self.get_access_token().await?;
        let url = format!("{}/{}/{}", self.base_url(), collection, doc_id);

        let response = self.client.delete(&url).bearer_auth(&token).send().await?;

        if !response.status().is_success() && response.status() != 404 {
            let status = response.status();
            let body = response.text().await?;
            debug!("Firebase delete error: {}", body);
            return Err(anyhow!("Firebase delete error: {}", status));
        }

        Ok(())
    }

    /// Get all users collection
    pub async fn get_all_users(&self) -> Result<Vec<Value>> {
        let token = self.get_access_token().await?;
        let url = format!("{}/users", self.base_url());

        let response = self.client.get(&url).bearer_auth(&token).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            debug!("Firebase error: {}", body);
            return Err(anyhow!("Firebase error: {}", status));
        }

        let result: Value = response.json().await?;
        let docs = result["documents"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|doc| {
                        let mut parsed = from_firestore_document(doc);
                        // Extract user ID from document name
                        if let Some(name) = doc["name"].as_str() {
                            if let Some(id) = name.split('/').last() {
                                parsed["_id"] = json!(id);
                            }
                        }
                        parsed
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(docs)
    }
}

/// Convert Firestore document to regular JSON
fn from_firestore_document(doc: &Value) -> Value {
    if let Some(fields) = doc.get("fields") {
        from_firestore_value(&json!({ "mapValue": { "fields": fields } }))
    } else {
        Value::Null
    }
}

/// Convert Firestore value to regular JSON value
fn from_firestore_value(value: &Value) -> Value {
    if let Some(s) = value.get("stringValue") {
        return s.clone();
    }
    if let Some(n) = value.get("integerValue") {
        if let Some(s) = n.as_str() {
            return Value::Number(s.parse().unwrap_or(0.into()));
        }
        return n.clone();
    }
    if let Some(n) = value.get("doubleValue") {
        return n.clone();
    }
    if let Some(b) = value.get("booleanValue") {
        return b.clone();
    }
    if let Some(ts) = value.get("timestampValue") {
        return ts.clone();
    }
    if value.get("nullValue").is_some() {
        return Value::Null;
    }
    if let Some(arr) = value
        .get("arrayValue")
        .and_then(|a| a.get("values"))
        .and_then(|v| v.as_array())
    {
        return Value::Array(arr.iter().map(from_firestore_value).collect());
    }
    if let Some(obj) = value
        .get("mapValue")
        .and_then(|m| m.get("fields"))
        .and_then(|f| f.as_object())
    {
        let map: serde_json::Map<String, Value> = obj
            .iter()
            .map(|(k, v)| (k.clone(), from_firestore_value(v)))
            .collect();
        return Value::Object(map);
    }
    Value::Null
}

/// Convert regular JSON to Firestore document format
fn to_firestore_document(data: &Value) -> Value {
    json!({
        "fields": to_firestore_fields(data)
    })
}

/// Convert JSON object to Firestore fields
fn to_firestore_fields(data: &Value) -> Value {
    if let Some(obj) = data.as_object() {
        let fields: serde_json::Map<String, Value> = obj
            .iter()
            .map(|(k, v)| (k.clone(), to_firestore_value(v)))
            .collect();
        Value::Object(fields)
    } else {
        json!({})
    }
}

/// Convert JSON value to Firestore value format
fn to_firestore_value(value: &Value) -> Value {
    match value {
        Value::String(s) => json!({ "stringValue": s }),
        Value::Number(n) => {
            if n.is_f64() {
                json!({ "doubleValue": n })
            } else {
                json!({ "integerValue": n.to_string() })
            }
        }
        Value::Bool(b) => json!({ "booleanValue": b }),
        Value::Array(arr) => {
            let values: Vec<Value> = arr.iter().map(to_firestore_value).collect();
            json!({ "arrayValue": { "values": values } })
        }
        Value::Object(obj) => {
            let fields: serde_json::Map<String, Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), to_firestore_value(v)))
                .collect();
            json!({ "mapValue": { "fields": fields } })
        }
        Value::Null => json!({ "nullValue": null }),
    }
}
