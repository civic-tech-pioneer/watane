use crate::outcome::{Code, Problem};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use watane_host::{InstalledMod, RunInfo};
pub struct Client {
    port: u16,
}
impl Client {
    pub fn for_home(home: &Path) -> Result<Option<Self>, Problem> {
        let Some(run) = RunInfo::read(home).map_err(Problem::internal)? else {
            return Ok(None);
        };
        if run.watane != 1 || run.mode != "preview" {
            return Ok(None);
        }
        let client = Self { port: run.port };
        let status = match client.request("status", None, Duration::from_secs(2)) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        if status["watane"] == 1
            && status["mode"] == "preview"
            && status["home"] == json!(home)
            && status["port"] == run.port
        {
            Ok(Some(client))
        } else {
            Ok(None)
        }
    }
    fn request(
        &self,
        path: &str,
        body: Option<Value>,
        timeout: Duration,
    ) -> Result<Value, Problem> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(None)
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .max_redirects(0)
            .build()
            .into();
        let url = format!("http://localhost:{}/api/{path}", self.port);
        let response = if let Some(body) = body {
            agent.post(&url).send_json(body)
        } else {
            agent.get(&url).call()
        };
        let mut response = response.map_err(|e| {
            Problem::new(Code::HostNoAnswer, e.to_string())
                .fix("Check the preview log and run the command again")
        })?;
        let status = response.status();
        let value: Value = response.body_mut().read_json().map_err(Problem::internal)?;
        if !status.is_success() {
            let code = if value["error"]["code"] == "refused" {
                Code::HostDeclined
            } else {
                Code::HostRefused
            };
            return Err(Problem::new(
                code,
                format!("[{}] {}", value["error"]["code"], value["error"]["message"]),
            )
            .fix("Correct the reported host error and run the command again"));
        }
        Ok(value)
    }
    pub fn install(&self, folders: &[PathBuf]) -> Result<Vec<InstalledMod>, Problem> {
        #[derive(Deserialize)]
        struct Installed {
            installed: Vec<InstalledMod>,
        }
        let value = self.request(
            "install",
            Some(json!({"folders":folders})),
            Duration::from_secs(60),
        )?;
        serde_json::from_value::<Installed>(value)
            .map(|v| v.installed)
            .map_err(Problem::internal)
    }
}
