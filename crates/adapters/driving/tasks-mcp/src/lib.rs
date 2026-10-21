//! The tasks MCP server: `taskloom mcp`, the tools `task_list` and
//! `task_add` over stdio, each call translated into the application's use
//! cases and its answer given in Go taskloom's shapes. The server's
//! instructions, and each tool's description and input and output
//! schema, are Go's, copied verbatim; a parity test holds them to the Go
//! source.
//!
//! A call is served by a [`TaskService`] of its own, made for it by the
//! composition root, on a blocking thread: two calls share nothing in the
//! process, so one waiting (on the registry's lock, or in an add's
//! retries) never holds up another, and in-process calls contend only as
//! separate processes do, in the store. The service resolves its project
//! synchronously, blocking on the registry's lock, which is why the call
//! runs where blocking is allowed.

mod error;
mod tools;

use std::future::Future;
use std::sync::Arc;

use clap::Subcommand;
use presentation::Catalog;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::IntoTransport;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use tasks_application::TaskService;

pub use error::{diagnostic_samples, McpError, McpNotice, EN_US};

/// The server's instructions to its client, Go taskloom's
/// (`mcpServerInstructions`) verbatim.
pub const INSTRUCTIONS: &str = include_str!("instructions.txt");

/// The tools this server offers, as `tools/list` answers: Go taskloom's
/// `task_add` and `task_list` (name, description, input and output schema)
/// verbatim, in Go's order.
pub const TOOLS: &str = include_str!("tools.json");

/// The command that serves the tools, for the binary to mount.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Group {
    /// Serve the task tools over MCP on stdio
    Mcp,
}

/// What every call is made with, read once when the server starts.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Settings {
    /// `--project`: the project every call is about, overriding
    /// resolution.
    pub project: Option<String>,
    /// CTXLOOM_SESSION_HARP: the session a task is added in.
    pub session: String,
    /// The binary's name and version, as the server names itself.
    pub name: String,
    pub version: String,
}

/// The server: tools over use cases, each call served by a service
/// `services` makes for it.
pub struct TaskTools<F> {
    services: Arc<F>,
    settings: Arc<Settings>,
    catalog: Arc<Catalog>,
    tools: Vec<Tool>,
}

impl<F> TaskTools<F> {
    /// The server over `services`, rendering its errors from `catalog`.
    pub fn new(services: F, settings: Settings, catalog: Arc<Catalog>) -> TaskTools<F> {
        TaskTools {
            services: Arc::new(services),
            settings: Arc::new(settings),
            catalog,
            tools: tools::offered(),
        }
    }
}

impl<F, S> TaskTools<F>
where
    F: Fn() -> S + Send + Sync + 'static,
    S: TaskService + Send + 'static,
{
    /// Serve over stdin and stdout, rmcp's stdio transport, until the
    /// client goes away.
    // Untestable here: stdin and stdout would be the test process's own.
    // The binary's acceptance test (taskloom's mcp.test.rs) serves
    // through it.
    #[cfg_attr(test, mutants::skip)]
    pub async fn serve_stdio(self) -> Result<(), McpError> {
        self.serve(rmcp::transport::stdio()).await
    }

    /// Serve over `transport` until the client goes away.
    pub async fn serve<T, E, A>(self, transport: T) -> Result<(), McpError>
    where
        T: IntoTransport<RoleServer, E, A>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let running = ServiceExt::serve(self, transport)
            .await
            .map_err(|e| McpError::Serve {
                detail: e.to_string(),
            })?;
        running
            .waiting()
            .await
            .map(|_| ())
            .map_err(|e| McpError::Serve {
                detail: e.to_string(),
            })
    }

    /// `call` on a service of its own, on a blocking thread, its future
    /// driven there to its end.
    async fn on_its_own<T, Fut>(
        &self,
        call: impl FnOnce(S) -> Fut + Send + 'static,
    ) -> Result<T, McpError>
    where
        Fut: Future<Output = T>,
        T: Send + 'static,
    {
        let services = Arc::clone(&self.services);
        let runtime = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || runtime.block_on(call(services())))
            .await
            .map_err(|e| McpError::Crashed {
                detail: e.to_string(),
            })
    }

    /// The tool `name` called with `arguments`: its result, or why not.
    async fn call(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        match name {
            tools::TASK_LIST => {
                let query = tools::list_query(arguments, &self.settings)?;
                let listing = self
                    .on_its_own(move |mut service| async move { service.list(query).await })
                    .await?
                    .map_err(McpError::App)?;
                Ok(tools::listed(&listing, &self.catalog))
            }
            tools::TASK_ADD => {
                let task = tools::new_task(arguments, &self.settings)?;
                let written = self
                    .on_its_own(move |mut service| async move { service.add(task).await })
                    .await?
                    .map_err(McpError::App)?;
                Ok(tools::added(&written))
            }
            other => Err(McpError::UnknownTool {
                name: other.to_string(),
            }),
        }
    }
}

impl<F, S> ServerHandler for TaskTools<F>
where
    F: Fn() -> S + Send + Sync + 'static,
    S: TaskService + Send + 'static,
{
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                self.settings.name.clone(),
                self.settings.version.clone(),
            ))
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tools.clone()))
    }

    /// The tool's result; its error, a tool error carrying the
    /// diagnostic's envelope. An unknown tool is a protocol error.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let arguments = serde_json::Value::Object(request.arguments.unwrap_or_default());
        let result = match self.call(&request.name, arguments).await {
            Ok(value) => CallToolResult::structured(value),
            Err(error @ McpError::UnknownTool { .. }) => {
                let envelope = serde_json::to_value(self.catalog.envelope(&error)).ok();
                return Err(ErrorData::invalid_params(
                    self.catalog.message(&error),
                    envelope,
                ));
            }
            Err(error) => CallToolResult::error(vec![ContentBlock::text(envelope_text(
                &self.catalog,
                &error,
            ))]),
        };
        Ok(result.into())
    }
}

/// `error`'s envelope, as one line of JSON.
fn envelope_text(catalog: &Catalog, error: &McpError) -> String {
    serde_json::to_string(&catalog.envelope(error)).unwrap_or_else(|_| catalog.message(error))
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;

#[cfg(test)]
#[path = "fakes.test.rs"]
mod fakes;
