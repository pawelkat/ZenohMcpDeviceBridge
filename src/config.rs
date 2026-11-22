use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub connect: String,
    pub mcp_listen: String,
    #[serde(default)]
    pub device: Vec<Device>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Device {
    pub id: String,
    pub title: String,
    pub fw: String,
    #[serde(default)]
    pub state: Vec<String>,
    pub command: Vec<String>,
    #[serde(default)]
    pub tool: Vec<Tool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub confirm: bool,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        toml::from_str(&text).map_err(|e| format!("{path}: {e}"))
    }
}

impl Device {
    pub fn tool(&self, name: &str) -> Option<&Tool> {
        self.tool.iter().find(|tool| tool.name == name)
    }

    pub fn manifest(&self) -> String {
        let tools: Vec<_> = self
            .tool
            .iter()
            .map(|tool| {
                let required = if tool.confirm {
                    r#","required":["confirm"]"#
                } else {
                    ""
                };
                let confirm_prop = if tool.confirm {
                    r#","properties":{"confirm":{"type":"boolean"}}"#
                } else {
                    r#","properties":{}"#
                };
                format!(
                    r#"{{"name":{},"description":{},"inputSchema":{{"type":"object"{confirm_prop}{required}}}}}"#,
                    json_string(&tool.name),
                    json_string(&tool.description),
                )
            })
            .collect();
        let state: Vec<_> = self.state.iter().map(|name| json_string(name)).collect();
        format!(
            r#"{{"schemaVersion":1,"device":{},"title":{},"fw":{},"tools":[{}],"state":[{}]}}"#,
            json_string(&self.id),
            json_string(&self.title),
            json_string(&self.fw),
            tools.join(","),
            state.join(","),
        )
    }
}

pub fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
