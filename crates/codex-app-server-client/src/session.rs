use std::time::Duration;

use crate::protocol::{
    ClientInfo, InitializeCapabilities, InitializeResponse, ThreadStartParams, ThreadStartResponse,
    TurnStartParams, TurnStartResponse,
};
use crate::transport::{AsyncBufRead, AsyncWrite};
use crate::{ApprovalHandler, CodexAppServerClient, Event, EventCollector, EventStream, Result};

#[derive(Clone, Debug)]
pub struct SessionOptions {
    pub client_info: ClientInfo,
    pub capabilities: Option<InitializeCapabilities>,
    pub command: String,
    pub extra_args: Vec<String>,
    pub call_timeout: Option<Duration>,
}

impl SessionOptions {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            client_info: ClientInfo::new(name, version),
            capabilities: None,
            command: "codex".to_owned(),
            extra_args: Vec::new(),
            call_timeout: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.client_info = self.client_info.with_title(title);
        self
    }

    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = command.into();
        self
    }

    pub fn with_extra_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    pub fn with_config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_args.push("-c".to_owned());
        self.extra_args
            .push(format!("{}={}", key.into(), value.into()));
        self
    }

    pub fn with_capabilities(mut self, capabilities: InitializeCapabilities) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    pub fn with_call_timeout(mut self, timeout: Duration) -> Self {
        self.call_timeout = Some(timeout);
        self
    }
}

pub struct CodexSession {
    client: CodexAppServerClient,
    events: EventStream,
    initialize_response: InitializeResponse,
}

impl CodexSession {
    pub async fn spawn(options: SessionOptions) -> Result<Self> {
        let (client, events) = CodexAppServerClient::spawn(&options.command, &options.extra_args)?;
        Self::handshake(client, events, options).await
    }

    pub async fn connect_streams<R, W>(
        reader: R,
        writer: W,
        options: SessionOptions,
    ) -> Result<Self>
    where
        R: AsyncBufRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (client, events) = CodexAppServerClient::connect_streams(reader, writer);
        Self::handshake(client, events, options).await
    }

    #[cfg(unix)]
    pub async fn connect_unix(
        path: impl AsRef<std::path::Path>,
        options: SessionOptions,
    ) -> Result<Self> {
        let (client, events) = CodexAppServerClient::connect_unix(path).await?;
        Self::handshake(client, events, options).await
    }

    async fn handshake(
        client: CodexAppServerClient,
        events: EventStream,
        options: SessionOptions,
    ) -> Result<Self> {
        let client = if let Some(timeout) = options.call_timeout {
            client.with_call_timeout(timeout)
        } else {
            client
        };
        let initialize_response = client
            .initialize(crate::protocol::InitializeParams {
                capabilities: options.capabilities,
                client_info: options.client_info,
            })
            .await?;
        client.send_initialized()?;
        Ok(Self {
            client,
            events,
            initialize_response,
        })
    }

    pub fn client(&self) -> &CodexAppServerClient {
        &self.client
    }

    pub fn initialize_response(&self) -> &InitializeResponse {
        &self.initialize_response
    }

    pub async fn next_event(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    pub async fn next_notification<H>(
        &mut self,
        handler: &H,
    ) -> Option<crate::protocol::ServerNotification>
    where
        H: ApprovalHandler,
    {
        loop {
            match self.events.recv().await? {
                Event::Notification(notification) => return Some(notification),
                Event::Request(request) => {
                    let reply = handler.handle(&request.request);
                    let _ = reply.send(request);
                }
                Event::Closed => return None,
            }
        }
    }

    pub async fn start_thread(&self, params: ThreadStartParams) -> Result<ThreadStartResponse> {
        self.client.thread_start(params).await
    }

    pub async fn send_turn(&self, params: TurnStartParams) -> Result<TurnStartResponse> {
        self.client.turn_start(params).await
    }

    pub async fn collect_until_complete<H>(
        &mut self,
        collector: &mut EventCollector,
        handler: &H,
    ) -> Result<()>
    where
        H: ApprovalHandler,
    {
        while !collector.is_complete() {
            let Some(notification) = self.next_notification(handler).await else {
                break;
            };
            collector.observe_notification(&notification);
        }
        Ok(())
    }
}
