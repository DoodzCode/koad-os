use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest::blocking::Client;
use anyhow::{Result, anyhow};

pub struct AirtableClient {
    client: Client,
    pat: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirtableRecord {
    pub id: String,
    pub fields: Value,
    #[serde(rename = "createdTime")]
    pub created_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirtableResponse {
    pub records: Vec<AirtableRecord>,
    pub offset: Option<String>,
}

impl AirtableClient {
    pub fn new(pat: String) -> Self {
        Self {
            client: Client::new(),
            pat,
        }
    }

    pub fn list_records(&self, base_id: &str, table_name: &str, filter: Option<String>, limit: Option<usize>) -> Result<Vec<AirtableRecord>> {
        let url = format!("https://api.airtable.com/v0/{}/{}", base_id, table_name);
        let mut request = self.client.get(&url)
            .bearer_auth(&self.pat);

        if let Some(f) = filter {
            request = request.query(&[("filterByFormula", f)]);
        }

        if let Some(l) = limit {
            request = request.query(&[("maxRecords", l)]);
        }

        let response = request.send()?;
        if !response.status().is_success() {
            return Err(anyhow!("Airtable List Error: {} - {}", response.status(), response.text()?));
        }

        let data: AirtableResponse = response.json()?;
        Ok(data.records)
    }

    pub fn get_record(&self, base_id: &str, table_name: &str, record_id: &str) -> Result<AirtableRecord> {
        let url = format!("https://api.airtable.com/v0/{}/{}/{}", base_id, table_name, record_id);
        let response = self.client.get(&url)
            .bearer_auth(&self.pat)
            .send()?;

        if !response.status().is_success() {
            return Err(anyhow!("Airtable Get Error: {} - {}", response.status(), response.text()?));
        }

        let record: AirtableRecord = response.json()?;
        Ok(record)
    }

    pub fn create_record(&self, base_id: &str, table_name: &str, fields: Value) -> Result<AirtableRecord> {
        let url = format!("https://api.airtable.com/v0/{}/{}", base_id, table_name);
        let body = serde_json::json!({ "fields": fields });

        let response = self.client.post(&url)
            .bearer_auth(&self.pat)
            .json(&body)
            .send()?;

        if !response.status().is_success() {
            return Err(anyhow!("Airtable Create Error: {} - {}", response.status(), response.text()?));
        }

        let record: AirtableRecord = response.json()?;
        Ok(record)
    }

    pub fn update_record(&self, base_id: &str, table_name: &str, record_id: &str, fields: Value) -> Result<AirtableRecord> {
        let url = format!("https://api.airtable.com/v0/{}/{}/{}", base_id, table_name, record_id);
        let body = serde_json::json!({ "fields": fields });

        let response = self.client.patch(&url)
            .bearer_auth(&self.pat)
            .json(&body)
            .send()?;

        if !response.status().is_success() {
            return Err(anyhow!("Airtable Update Error: {} - {}", response.status(), response.text()?));
        }

        let record: AirtableRecord = response.json()?;
        Ok(record)
    }
}

pub fn escape_formula_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\'', "\\\'")
        .replace('\0', "")
        .chars()
        .filter(|&c| !c.is_control())
        .collect()
}

pub fn equals_formula(field: &str, value: &str) -> String {
    format!("{{{}}} = '{}'", field, escape_formula_value(value))
}
