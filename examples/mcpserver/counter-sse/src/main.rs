use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};
use poem_mcpserver::{sse::sse_endpoint, tool::Text, McpServer, Tools};

struct Counter {
    count: i32,
}

/// This server provides a counter tool that can increment and decrement values.
///
/// The counter starts at 0 and can be modified using the 'increment' and
/// 'decrement' tools. Use 'get_value' to check the current count.
#[Tools]
impl Counter {
    /// Increment the counter by 1
    async fn increment(&mut self) -> Text<i32> {
        self.count += 1;
        Text(self.count)
    }

    /// Decrement the counter by 1
    async fn decrement(&mut self) -> Text<i32> {
        self.count -= 1;
        Text(self.count)
    }

    /// Get the current counter value
    async fn get_value(&self) -> Text<i32> {
        Text(self.count)
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8000");
    let app = Route::new()
        .at(
            "/sse",
            sse_endpoint(|_| McpServer::new().tools(Counter { count: 0 })),
        )
        .with(Cors::new());
    Server::new(listener).run(app).await
}
