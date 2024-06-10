use std::error::Error;

use nacos_sdk::api::config::{ConfigService, ConfigServiceBuilder};
use nacos_sdk::api::props::ClientProps;

use crate::config::Nacos;

pub fn build_config_service(nacos: &Nacos) -> Result<impl ConfigService, Box<dyn std::error::Error>> {
    let nacos_from = ClientProps::new()
        .server_addr(nacos.addr.clone())
        .namespace(nacos.namespace.clone())
        .auth_username(nacos.username.clone().unwrap())
        .auth_password(nacos.password.clone().unwrap());
    let config_service = ConfigServiceBuilder::new(nacos_from.clone())
        .enable_auth_plugin_http().build()?;
    Ok(config_service)
}

pub async fn get_all_data_id(from_nacos: &Nacos) -> Result<Vec<String>, Box<dyn Error>> {
    // 创建一个 reqwest 客户端
    let client = reqwest::Client::new();
    let login_response = client.post(format!("http://{}/nacos/v1/auth/users/login?message=true", from_nacos.addr.clone()))
        .form(&[("username", from_nacos.username.clone().unwrap()), ("password", from_nacos.password.clone().unwrap())])
        .send()
        .await?;
    login_response.status().is_success()
        .then(|| ())
        .ok_or_else(|| format!("Failed to login to Nacos: {}", login_response.status()))?;
    // 获取响应内容
    let body = login_response.text().await?;

    let token_json: serde_json::Value = serde_json::from_str(&body)?;
    let access_token = token_json.get("accessToken").unwrap().as_str().unwrap();

    let config_url = format!(
        "http://{}/nacos/v1/cs/configs?dataId=&group=&appName=&config_tags=&search=accurate&message=true&tenant={}&accessToken={}&pageNo=1&pageSize=10000",
        &from_nacos.addr, &from_nacos.namespace, &access_token);
    let config_list_response = client.get(config_url)
        .header("accessToken", access_token)
        .send()
        .await?;

    config_list_response.status().is_success()
        .then(|| ())
        .ok_or_else(|| format!("Failed to get sync_config list from Nacos: {}", config_list_response.status()))?;

    let config_list_response_body = config_list_response.text().await?;
    let config_list_json: serde_json::Value = serde_json::from_str(&config_list_response_body)?;

    let result = config_list_json
        .get("pageItems")
        .and_then(|items| items.as_array())
        .map(|items| {
            items.iter()
                .filter_map(|item| {
                    item.get("type")
                        .and_then(|t| t.as_str())
                        .filter(|&t| t == "yaml")
                        .and_then(|_| item.get("dataId"))
                        .and_then(|id| id.as_str())
                        .map(|id| id.to_string())
                })
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    Ok(result)
}