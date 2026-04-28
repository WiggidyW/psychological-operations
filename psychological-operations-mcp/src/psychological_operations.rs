use rmcp::{
    ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PsychologicalOperationsRequest {
    #[schemars(description = "The command arguments to pass to the psychological-operations CLI (e.g. [\"psyops\", \"list\"] or [\"scrapes\", \"run\", \"--name\", \"foo\"])")]
    pub command: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PsychologicalOperationsMcpCli {
    pub tool_router: ToolRouter<Self>,
}

#[tool_router]
impl PsychologicalOperationsMcpCli {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "Psychological Operations CLI",
        description = "Run a psychological-operations CLI command."
    )]
    async fn psychological_operations(
        &self,
        Parameters(req): Parameters<PsychologicalOperationsRequest>,
    ) -> String {
        let args: Vec<String> = std::iter::once("psychological-operations".to_string())
            .chain(req.command)
            .collect();

        match psychological_operations_cli::run(args).await {
            Ok(output) => output,
            Err(e) => format!("error: {e}"),
        }
    }
}

#[tool_handler]
impl ServerHandler for PsychologicalOperationsMcpCli {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "psychological-operations-cli".into(),
                title: None,
                version: env!("CARGO_PKG_VERSION").into(),
                description: None,
                icons: None,
                website_url: None,
            },
            instructions: None,
        }
    }
}
