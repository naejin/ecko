//! MCP server -- expose ecko checks via the Model Context Protocol.

pub mod tools;

use rmcp::schemars::JsonSchema;
use rmcp::serde::{Deserialize, Serialize};
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::*,
    service::ServiceExt,
    tool, tool_handler, tool_router,
    transport::io::stdio,
    ErrorData as McpError, ServerHandler,
};

// --- Parameter types ---

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CheckFileParams {
    /// Absolute path to the file to check
    pub file_path: String,
    /// Working directory (project root)
    pub cwd: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct WorkspaceParams {
    /// Working directory (project root)
    pub cwd: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DryRunParams {
    /// Absolute path to the file
    pub file_path: String,
    /// Working directory (project root)
    pub cwd: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ExplainParams {
    /// Check name (e.g., 'bare-except', 'unused-imports')
    pub check_name: String,
}

// --- Server ---

#[derive(Clone)]
#[allow(dead_code)] // tool_router is used by the #[tool_router] macro at runtime
pub struct EckoMcpServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl EckoMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "ecko_check_file",
        description = "Run ecko code quality checks on a file and return echoes with fix suggestions"
    )]
    async fn check_file(
        &self,
        params: Parameters<CheckFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tools::check_file(&params.0.file_path, &params.0.cwd);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        name = "ecko_check_workspace",
        description = "Run ecko checks on all modified files in the workspace"
    )]
    async fn check_workspace(
        &self,
        params: Parameters<WorkspaceParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tools::check_workspace(&params.0.cwd);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        name = "ecko_status",
        description = "Show ecko configuration, available checks, and language support"
    )]
    async fn status(
        &self,
        params: Parameters<WorkspaceParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tools::status(&params.0.cwd);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        name = "ecko_dry_run",
        description = "List which checks would run on a file without executing them"
    )]
    async fn dry_run(&self, params: Parameters<DryRunParams>) -> Result<CallToolResult, McpError> {
        let result = tools::dry_run(&params.0.file_path, &params.0.cwd);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        name = "ecko_explain",
        description = "Explain what a specific ecko check does and why it matters"
    )]
    async fn explain(&self, params: Parameters<ExplainParams>) -> Result<CallToolResult, McpError> {
        let result = tools::explain(&params.0.check_name);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[tool_handler]
impl ServerHandler for EckoMcpServer {
    fn get_info(&self) -> InitializeResult {
        let mut result =
            InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
                .with_instructions(
                    "Ecko -- deterministic code quality checks for AI agents. \
                 Use ecko_check_file to lint a file, ecko_status to see config, \
                 ecko_dry_run to preview checks, ecko_explain to understand a check.",
                );
        result.server_info.name = "ecko".to_string();
        result.server_info.version = env!("CARGO_PKG_VERSION").to_string();
        result
    }
}

/// Start the MCP server with stdio transport.
pub async fn run_mcp_server() -> Result<(), Box<dyn std::error::Error>> {
    let server = EckoMcpServer::new();
    let transport = stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}
