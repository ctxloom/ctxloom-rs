//! The tasks MCP server: `taskloom mcp`, the tools `task_list`,
//! `task_add`, `task_set_status`, `task_edit` and `task_tag` and the
//! resources `taskloom://tag-schema` and `taskloom://tag-vocabulary` over
//! stdio, each call translated into the application's use cases and its
//! answer given in Go taskloom's shapes. The server's instructions and
//! capabilities, each tool's description and input and output schema,
//! and each resource's description, are Go's, copied verbatim but for the
//! human's reviewed factual changes to the instructions and the tool
//! descriptions (`fixtures/reviewed.rs`); parity tests hold them to the Go
//! source with those applied.
//!
//! A call is served by a [`TaskService`] of its own, made for it by the
//! composition root, on a blocking thread: two calls share nothing in the
//! process, so one waiting (on the registry's lock, or in an add's
//! retries) never holds up another, and in-process calls contend only as
//! separate processes do, in the store. The service resolves its project
//! synchronously, blocking on the registry's lock, which is why the call
//! runs where blocking is allowed.

mod error;
mod resources;
mod tools;

use std::future::Future;
use std::sync::Arc;

use clap::Subcommand;
use presentation::Catalog;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListResourcesResult, ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams,
    ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, ServerCapabilities,
    ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::IntoTransport;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use tasks_application::TaskService;

pub use error::{diagnostic_samples, McpError, McpNotice, EN_US};
pub use resources::RESOURCES;

/// The server's instructions to its client, Go taskloom's
/// (`mcpServerInstructions`) with the reviewed factual changes
/// (`fixtures/reviewed.rs`).
pub const INSTRUCTIONS: &str = include_str!("instructions.txt");

/// The tools this server offers, as `tools/list` answers: Go taskloom's
/// five (name, description, input and output schema) in Go's order,
/// verbatim but for the reviewed factual changes to the descriptions
/// (`fixtures/reviewed.rs`).
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
    resources: Vec<Resource>,
}

impl<F> TaskTools<F> {
    /// The server over `services`, rendering its errors from `catalog`.
    pub fn new(services: F, settings: Settings, catalog: Arc<Catalog>) -> TaskTools<F> {
        TaskTools {
            services: Arc::new(services),
            settings: Arc::new(settings),
            catalog,
            tools: tools::offered(),
            resources: resources::offered(),
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
        let settings = &self.settings;
        let written = match name {
            tools::TASK_LIST => return self.list(arguments).await,
            tools::TASK_ADD => {
                let task = tools::new_task(arguments, settings)?;
                self.on_its_own(move |mut service| async move { service.add(task).await })
                    .await?
            }
            tools::TASK_SET_STATUS => {
                let change = tools::status_change(arguments, settings)?;
                self.on_its_own(move |mut service| async move { service.set_status(change).await })
                    .await?
            }
            tools::TASK_EDIT => {
                let edit = tools::text_edit(arguments, settings)?;
                self.on_its_own(move |mut service| async move { service.edit(edit).await })
                    .await?
            }
            tools::TASK_TAG => {
                let change = tools::tag_change(arguments, settings)?;
                self.on_its_own(move |mut service| async move { service.tag(change).await })
                    .await?
            }
            other => {
                return Err(McpError::UnknownTool {
                    name: other.to_string(),
                })
            }
        };
        Ok(tools::written(&written.map_err(McpError::App)?))
    }

    /// `task_list`: the listing, and with `include_summary` the summary of
    /// what it reached, both on the call's one service.
    async fn list(&self, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let request = tools::list_request(arguments, &self.settings)?;
        let compact = request.compact;
        let (listing, summary) = self
            .on_its_own(move |mut service| async move {
                let listing = service.list(request.query.clone()).await?;
                let summary = match request.summary {
                    true => Some(
                        service
                            .summary(request.summary_scope(&listing.reach))
                            .await?,
                    ),
                    false => None,
                };
                Ok((listing, summary))
            })
            .await?
            .map_err(McpError::App)?;
        Ok(tools::listed(
            &listing,
            compact,
            summary.as_ref(),
            &self.catalog,
        ))
    }

    /// The resource at `uri`, read now: its document, or why not.
    async fn read(&self, uri: &str) -> Result<String, McpError> {
        match uri {
            resources::TAG_SCHEMA => {
                let schema = self
                    .on_its_own(|mut service| async move { service.tag_schema(Vec::new()).await })
                    .await?
                    .map_err(McpError::App)?;
                Ok(resources::schema_text(&schema))
            }
            resources::TAG_VOCABULARY => {
                let query = resources::vocabulary_query(&self.settings);
                let vocabulary = self
                    .on_its_own(move |mut service| async move { service.tags(query).await })
                    .await?
                    .map_err(McpError::App)?;
                Ok(resources::vocabulary_text(&vocabulary, &self.catalog))
            }
            other => Err(McpError::UnknownResource {
                uri: other.to_string(),
            }),
        }
    }

    /// `error` as a protocol error: its catalog message, its envelope the
    /// data.
    fn protocol_error(
        &self,
        error: &McpError,
        kind: fn(String, Option<serde_json::Value>) -> ErrorData,
    ) -> ErrorData {
        let envelope = serde_json::to_value(self.catalog.envelope(error)).ok();
        kind(self.catalog.message(error), envelope)
    }
}

impl<F, S> ServerHandler for TaskTools<F>
where
    F: Fn() -> S + Send + Sync + 'static,
    S: TaskService + Send + 'static,
{
    /// Go's capabilities: logging, and the tools and resources, each
    /// list said to change (though neither does).
    fn get_info(&self) -> ServerConfig {
        // Go advertises logging; rmcp deprecates it (SEP-2577) but still
        // serves it.
        #[allow(deprecated)]
        let capabilities = ServerCapabilities::builder()
            .enable_logging()
            .enable_resources()
            .enable_resources_list_changed()
            .enable_tools()
            .enable_tool_list_changed()
            .build();
        ServerConfig::new(capabilities)
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
                return Err(self.protocol_error(&error, ErrorData::invalid_params));
            }
            Err(error) => CallToolResult::error(vec![ContentBlock::text(envelope_text(
                &self.catalog,
                &error,
            ))]),
        };
        Ok(result.into())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(self.resources.clone()))
    }

    /// The resource's document, as JSON text; an unknown URI is a
    /// resource-not-found error, any other failure an internal one, each
    /// carrying the diagnostic's envelope.
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        match self.read(&request.uri).await {
            Ok(text) => {
                let contents =
                    ResourceContents::text(text, request.uri).with_mime_type(resources::JSON);
                Ok(ReadResourceResult::new(vec![contents]).into())
            }
            Err(error @ McpError::UnknownResource { .. }) => {
                Err(self.protocol_error(&error, ErrorData::resource_not_found))
            }
            Err(error) => Err(self.protocol_error(&error, ErrorData::internal_error)),
        }
    }

    /// Any level is accepted, as Go's server accepts it: nothing is
    /// logged to the client at any level.
    // Logging is deprecated in rmcp (SEP-2577) but Go advertises it.
    #[allow(deprecated)]
    async fn set_level(
        &self,
        _request: rmcp::model::SetLevelRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        Ok(())
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
