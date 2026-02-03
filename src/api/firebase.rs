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

/// Filter for structured queries
#[derive(Debug, Clone)]
pub struct QueryFilter {
    /// Field path, e.g., "timestamps.created" or "activity.type"
    pub field: String,
    /// Operator: "EQUAL", "LESS_THAN", "LESS_THAN_OR_EQUAL", 
    /// "GREATER_THAN", "GREATER_THAN_OR_EQUAL", "NOT_EQUAL"
    pub op: String,
    /// Value in Firestore format (e.g., { "stringValue": "..." })
    pub value: Value,
}

impl QueryFilter {
    /// Create a new filter with a string value
    pub fn string_eq(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            op: "EQUAL".to_string(),
            value: json!({ "stringValue": value.into() }),
        }
    }

    /// Create a >= filter with a timestamp value (RFC3339 string)
    pub fn timestamp_gte(field: impl Into<String>, rfc3339: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            op: "GREATER_THAN_OR_EQUAL".to_string(),
            value: json!({ "timestampValue": rfc3339.into() }),
        }
    }
}

/// Write operation for transactions
#[derive(Debug, Clone)]
pub enum TransactionWrite {
    /// Delete a document by path (e.g., "users/123/immersion_logs/abc")
    Delete { document_path: String },
    /// Update specific fields in a document
    Update { document_path: String, fields: Value },
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

    // ============ Structured Queries ============

    /// Run a structured query on a subcollection with server-side filtering.
    /// Returns Vec<(doc_id, data)>.
    ///
    /// # Arguments
    /// * `parent_collection` - e.g., "users"
    /// * `parent_doc_id` - e.g., user ID
    /// * `subcollection` - e.g., "immersion_logs"
    /// * `filters` - List of (field_path, op, value) tuples
    /// * `order_by` - Optional (field_path, direction) where direction is "ASCENDING" or "DESCENDING"
    /// * `limit` - Max documents to return
    /// * `start_after` - Optional cursor (document values to start after)
    pub async fn run_query(
        &self,
        parent_collection: &str,
        parent_doc_id: &str,
        subcollection: &str,
        filters: Vec<QueryFilter>,
        order_by: Option<(&str, &str)>,
        limit: usize,
        start_after: Option<&Value>,
    ) -> Result<Vec<(String, Value)>> {
        let token = self.get_access_token().await?;
        
        // Parent path for the query
        let parent = format!(
            "projects/{}/databases/(default)/documents/{}/{}",
            self.service_account.project_id, parent_collection, parent_doc_id
        );
        let url = format!(
            "https://firestore.googleapis.com/v1/{}:runQuery",
            parent
        );

        // Build structuredQuery
        let mut query = json!({
            "from": [{ "collectionId": subcollection }],
            "limit": limit
        });

        // Add filters
        if !filters.is_empty() {
            let filter_clauses: Vec<Value> = filters
                .iter()
                .map(|f| {
                    json!({
                        "fieldFilter": {
                            "field": { "fieldPath": &f.field },
                            "op": &f.op,
                            "value": f.value.clone()
                        }
                    })
                })
                .collect();

            if filter_clauses.len() == 1 {
                query["where"] = filter_clauses.into_iter().next().unwrap();
            } else {
                query["where"] = json!({
                    "compositeFilter": {
                        "op": "AND",
                        "filters": filter_clauses
                    }
                });
            }
        }

        // Add orderBy
        if let Some((field, direction)) = order_by {
            query["orderBy"] = json!([{
                "field": { "fieldPath": field },
                "direction": direction
            }]);
        }

        // Add startAfter cursor
        if let Some(cursor_values) = start_after {
            query["startAt"] = json!({
                "values": [cursor_values.clone()],
                "before": false
            });
        }

        let body = json!({ "structuredQuery": query });

        let response = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            debug!("Firebase query error: {}", body);
            return Err(anyhow!("Firebase query error: {}", status));
        }

        // Response is an array of { document: {...} } or { readTime: ... }
        let results: Vec<Value> = response.json().await?;
        let mut docs = Vec::new();

        for item in results {
            if let Some(doc) = item.get("document") {
                if let Some(name) = doc["name"].as_str() {
                    let id = name.split('/').last().unwrap_or("").to_string();
                    let data = from_firestore_document(doc);
                    docs.push((id, data));
                }
            }
        }

        Ok(docs)
    }

    // ============ Transactions ============

    /// Begin a new Firestore transaction. Returns the transaction ID.
    pub async fn begin_transaction(&self) -> Result<String> {
        let token = self.get_access_token().await?;
        let url = format!(
            "https://firestore.googleapis.com/v1/projects/{}/databases/(default)/documents:beginTransaction",
            self.service_account.project_id
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&json!({}))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            debug!("Firebase beginTransaction error: {}", body);
            return Err(anyhow!("Firebase beginTransaction error: {}", status));
        }

        let result: Value = response.json().await?;
        let tx_id = result["transaction"]
            .as_str()
            .ok_or_else(|| anyhow!("No transaction ID in response"))?;

        Ok(tx_id.to_string())
    }

    /// Commit a transaction with a list of writes.
    /// All writes are applied atomically.
    pub async fn commit_transaction(
        &self,
        transaction_id: &str,
        writes: Vec<TransactionWrite>,
    ) -> Result<()> {
        let token = self.get_access_token().await?;
        let url = format!(
            "https://firestore.googleapis.com/v1/projects/{}/databases/(default)/documents:commit",
            self.service_account.project_id
        );

        let write_objects: Vec<Value> = writes
            .into_iter()
            .map(|w| match w {
                TransactionWrite::Delete { document_path } => {
                    let full_path = format!(
                        "projects/{}/databases/(default)/documents/{}",
                        self.service_account.project_id, document_path
                    );
                    json!({ "delete": full_path })
                }
                TransactionWrite::Update { document_path, fields } => {
                    let full_path = format!(
                        "projects/{}/databases/(default)/documents/{}",
                        self.service_account.project_id, document_path
                    );
                    let field_paths: Vec<String> = fields
                        .as_object()
                        .map(|obj| obj.keys().cloned().collect())
                        .unwrap_or_default();
                    json!({
                        "update": {
                            "name": full_path,
                            "fields": to_firestore_fields(&fields)
                        },
                        "updateMask": {
                            "fieldPaths": field_paths
                        }
                    })
                }
            })
            .collect();

        let body = json!({
            "transaction": transaction_id,
            "writes": write_objects
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            debug!("Firebase commit error: {}", body);
            return Err(anyhow!("Firebase commit error: {}", status));
        }

        Ok(())
    }

    /// Read a document within a transaction context.
    pub async fn get_document_in_transaction(
        &self,
        transaction_id: &str,
        collection: &str,
        doc_id: &str,
    ) -> Result<Option<Value>> {
        let token = self.get_access_token().await?;
        let url = format!(
            "{}/{}/{}?transaction={}",
            self.base_url(),
            collection,
            doc_id,
            transaction_id
        );

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
