use serde::{Deserialize, Serialize};

use std::sync::{Arc, Mutex};
use indexmap::IndexMap;
use anyhow::{Result, Context};
use reqwest::blocking::Client;
// use jsonwebtoken::{encode, Algorithm, EncodingKey, Header}; // Removed
// use chrono::Utc;

#[derive(Default, Clone)]
pub struct AppData {
    pub server_order: Vec<String>,
    // Server -> (Category Name -> List of Chars)
    pub servers: IndexMap<String, IndexMap<String, Vec<CharacterData>>>, 
}




#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterData {
    pub login: String,
    pub password: String,
    pub character: String,
    pub exe: String,
    pub use_launcher: bool,
    pub seed: String,
    pub window_name: String,
    pub vocation: String,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Deserialize)]
struct ServiceAccount {
    client_email: String,
    private_key: String,
}

#[derive(Serialize)]
struct Claims {
    iss: String,
    scope: String,
    aud: String,
    exp: usize,
    iat: usize,
}

#[derive(Clone)]
pub struct DataSource {
    client: Client,
    token: Arc<Mutex<String>>,
    token_exp: Arc<Mutex<usize>>,
    creds_path: String,
}

impl DataSource {
    pub fn new(_creds_path: &str) -> Self {
        Self {
            client: Client::new(),
            token: Arc::new(Mutex::new(String::new())),
            token_exp: Arc::new(Mutex::new(0)),
            creds_path: String::from(crate::API_URL), 
        }
    }

    // Returns: (Sorted List of Server Names, The Data Map)
    pub fn fetch_data(&self, _sheet_id: &str, _range: &str) -> Result<(Vec<String>, IndexMap<String, IndexMap<String, Vec<CharacterData>>>)> {
        let api_key = "rust_app_api_key_secret_123"; 
        let url = format!("{}/api/rust/data", self.creds_path);

        let res = self.client.get(&url)
            .header("x-api-key", api_key)
            .send()
            .context("Failed to connect to backend")?;

        let data: IndexMap<String, IndexMap<String, Vec<CharacterData>>> = res.json().context("Failed to parse API response")?;
        
        let mut server_order: Vec<String> = data.keys().cloned().collect();
        server_order.sort(); 
        Ok((server_order, data))
    }

    pub fn save_character(&self, server: &str, char: &CharacterData, category: Option<&str>) -> Result<()> {
        let api_key = "rust_app_api_key_secret_123";
        let url = format!("{}/api/rust/character", self.creds_path);

        #[derive(Serialize)]
        struct SaveReq<'a> {
            server: &'a str,
            data: &'a CharacterData,
            category: Option<&'a str>,
        }

        self.client.post(&url)
            .header("x-api-key", api_key)
            .json(&SaveReq { server, data: char, category })
            .send()
            .context("Failed to save character")?;
        Ok(())
    }

    pub fn delete_character(&self, server: &str, name: &str) -> Result<()> {
        let api_key = "rust_app_api_key_secret_123";
        let url = format!("{}/api/rust/character/delete", self.creds_path);

        #[derive(Serialize)]
        struct DeleteReq<'a> {
            server: &'a str,
            character: &'a str,
        }

        self.client.post(&url)
            .header("x-api-key", api_key)
            .json(&DeleteReq { server, character: name })
            .send()
            .context("Failed to delete character")?;
        Ok(())
    }

    pub fn add_to_category(&self, server: &str, category: &str, char_name: &str) -> Result<()> {
        let api_key = "rust_app_api_key_secret_123";
        let url = format!("{}/api/rust/category/add", self.creds_path);

        #[derive(Serialize)]
        struct CatReq<'a> {
            server: &'a str,
            category: &'a str,
            character: &'a str,
        }

        self.client.post(&url)
            .header("x-api-key", api_key)
            .json(&CatReq { server, category, character: char_name })
            .send()
            .context("Failed to add to category")?;
        Ok(())
    }

    pub fn remove_from_category(&self, server: &str, category: &str, char_name: &str) -> Result<()> {
        let api_key = "rust_app_api_key_secret_123";
        let url = format!("{}/api/rust/category/remove", self.creds_path);

        #[derive(Serialize)]
        struct CatReq<'a> {
            server: &'a str,
            category: &'a str,
            character: &'a str,
        }

        self.client.post(&url)
            .header("x-api-key", api_key)
            .json(&CatReq { server, category, character: char_name })
            .send()
            .context("Failed to remove from category")?;
        Ok(())
    }

    pub fn create_category(&self, server: &str, category: &str) -> Result<()> {
        let api_key = "rust_app_api_key_secret_123";
        let url = format!("{}/api/rust/category/create", self.creds_path);

        #[derive(Serialize)]
        struct CatCreateReq<'a> {
            server: &'a str,
            category: &'a str,
        }

        self.client.post(&url)
            .header("x-api-key", api_key)
            .json(&CatCreateReq { server, category })
            .send()
            .context("Failed to create category")?;
        Ok(())
    }

    pub fn delete_category(&self, server: &str, category: &str) -> Result<()> {
        let api_key = "rust_app_api_key_secret_123";
        let url = format!("{}/api/rust/category/delete", self.creds_path);

        #[derive(Serialize)]
        struct CatDeleteReq<'a> {
            server: &'a str,
            category: &'a str,
        }

        self.client.post(&url)
            .header("x-api-key", api_key)
            .json(&CatDeleteReq { server, category })
            .send()
            .context("Failed to delete category")?;
        Ok(())
    }

    pub fn reorder_categories(&self, server: &str, order: Vec<String>) -> Result<()> {
        let api_key = "rust_app_api_key_secret_123";
        let url = format!("{}/api/rust/category/reorder", self.creds_path);

        #[derive(Serialize)]
        struct CatReorderReq<'a> {
            server: &'a str,
            order: Vec<String>,
        }

        self.client.post(&url)
            .header("x-api-key", api_key)
            .json(&CatReorderReq { server, order })
            .send()
            .context("Failed to reorder categories")?;
        Ok(())
    }

    pub fn rename_category(&self, server: &str, old_category: &str, new_category: &str) -> Result<()> {
        let api_key = "rust_app_api_key_secret_123";
        let url = format!("{}/api/rust/category/rename", self.creds_path);

        #[derive(Serialize)]
        struct CatRenameReq<'a> {
            server: &'a str,
            old_category: &'a str,
            new_category: &'a str,
        }

        self.client.post(&url)
            .header("x-api-key", api_key)
            .json(&CatRenameReq { server, old_category, new_category })
            .send()
            .context("Failed to rename category")?;
        Ok(())
    }

    pub fn fetch_active_sessions(&self) -> Result<Vec<crate::login_tab::ActiveSession>> {
        let api_key = "rust_app_api_key_secret_123";
        let url = format!("{}/api/rust/sessions", self.creds_path);

        let res = self.client.get(&url)
            .header("x-api-key", api_key)
            .send()
            .context("Failed to connect to backend for sessions")?;

        let data: Vec<crate::login_tab::ActiveSession> = res.json().context("Failed to parse sessions")?;
        Ok(data)
    }
}
