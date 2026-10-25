use serde::{Deserialize, Serialize};

/// 搜索/lookup 返回的 App 信息（字段名对齐 iTunes Search API）
#[derive(Serialize, Deserialize)]
pub struct Software {
    #[serde(rename(deserialize = "trackId", serialize = "id"))]
    pub id: i64,
    #[serde(rename(deserialize = "bundleId", serialize = "bundle_id"))]
    pub bundle_id: String,
    #[serde(rename(deserialize = "trackName", serialize = "name"))]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(rename(deserialize = "artistName", serialize = "artist_name"), default)]
    pub artist_name: Option<String>,
    #[serde(
        rename(deserialize = "artworkUrl512", serialize = "artwork_url"),
        default
    )]
    pub artwork_url: Option<String>,
    #[serde(
        rename(deserialize = "minimumOsVersion", serialize = "minimum_os_version"),
        default
    )]
    pub minimum_os_version: Option<String>,
    #[serde(
        rename(deserialize = "formattedPrice", serialize = "formatted_price"),
        default
    )]
    pub formatted_price: Option<String>,
}
