use crate::{Result, Sage};
use sage_api::{
    DeleteUserTheme, DeleteUserThemeResponse, GetNftData, GetUserTheme, GetUserThemeResponse,
    GetUserThemes, GetUserThemesResponse, SaveUserTheme, SaveUserThemeResponse,
};
use serde_json::Value;
use tokio::fs;

impl Sage {
    pub async fn delete_user_theme(&self, req: DeleteUserTheme) -> Result<DeleteUserThemeResponse> {
        if req.nft_id.is_empty() {
            return Ok(DeleteUserThemeResponse {});
        }

        let themes_dir = self.path.join("themes");
        let theme_dir = themes_dir.join(&req.nft_id);

        if !theme_dir.exists() {
            return Ok(DeleteUserThemeResponse {});
        }

        fs::remove_dir_all(&theme_dir).await?;

        Ok(DeleteUserThemeResponse {})
    }

    pub async fn get_user_theme(&self, req: GetUserTheme) -> Result<GetUserThemeResponse> {
        if req.nft_id.is_empty() {
            return Ok(GetUserThemeResponse { theme: None });
        }

        let themes_dir = self.path.join("themes");
        let theme_dir = themes_dir.join(&req.nft_id);
        let theme_json_path = theme_dir.join("theme.json");

        if !theme_json_path.exists() {
            return Ok(GetUserThemeResponse { theme: None });
        }

        let theme_json = fs::read_to_string(&theme_json_path).await?;

        Ok(GetUserThemeResponse {
            theme: Some(theme_json),
        })
    }

    pub async fn save_user_theme(&self, req: SaveUserTheme) -> Result<SaveUserThemeResponse> {
        if req.nft_id.is_empty() {
            return Ok(SaveUserThemeResponse {});
        }

        let themes_dir = self.path.join("themes");

        if !themes_dir.exists() {
            fs::create_dir_all(&themes_dir).await?;
        }

        let nft_theme_dir = themes_dir.join(&req.nft_id);

        if !nft_theme_dir.exists() {
            fs::create_dir_all(&nft_theme_dir).await?;
        }

        // Get NFT data to extract the theme JSON
        let nft_data_response = self
            .get_nft_data(GetNftData {
                nft_id: req.nft_id.clone(),
            })
            .await?;

        let theme_json_path = nft_theme_dir.join("theme.json");

        if let Some(nft_data) = nft_data_response.data {
            if let Some(metadata_json) = nft_data.metadata_json {
                let json_value: Value = serde_json::from_str(&metadata_json)
                    .map_err(|_| crate::Error::InvalidThemeJson)?;

                // Extract the data.theme node
                let mut theme_data = json_value
                    .get("data")
                    .and_then(|data| data.get("theme"))
                    .ok_or(crate::Error::MissingThemeData)?
                    .clone();

                // Set the name property to the NFT ID
                if let Some(theme_obj) = theme_data.as_object_mut() {
                    theme_obj.insert(
                        "name".to_string(),
                        serde_json::Value::String(req.nft_id.clone()),
                    );
                }

                let theme_json = serde_json::to_string_pretty(&theme_data)
                    .map_err(|_| crate::Error::InvalidThemeJson)?;
                fs::write(&theme_json_path, theme_json).await?;
            }
        }

        Ok(SaveUserThemeResponse {})
    }

    pub async fn get_user_themes(&self, _req: GetUserThemes) -> Result<GetUserThemesResponse> {
        let themes_dir = self.path.join("themes");
        let mut themes = Vec::new();

        if !themes_dir.exists() {
            return Ok(GetUserThemesResponse { themes });
        }

        match fs::read_dir(&themes_dir).await {
            Ok(mut entries) => {
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();

                    if path.is_dir() {
                        let theme_json_path = path.join("theme.json");

                        if theme_json_path.exists() {
                            match fs::read_to_string(&theme_json_path).await {
                                Ok(theme_content) => {
                                    themes.push(theme_content);
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Failed to read theme.json in {}: {e}",
                                        path.display()
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to read themes directory: {e}");
            }
        }

        Ok(GetUserThemesResponse { themes })
    }
}
