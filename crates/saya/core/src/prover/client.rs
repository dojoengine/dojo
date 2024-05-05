use url::Url;

pub async fn http_prove(prover_url: Url, input: String) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let resp = client.post(prover_url).body(input).send().await?;
    let result = resp.text().await?;
    Ok(result)
}
