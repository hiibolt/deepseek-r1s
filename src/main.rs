use std::time::{SystemTime, UNIX_EPOCH};

use axum::{extract::{ws::{Message, WebSocket}, WebSocketUpgrade}, response::{Html, Response}, routing::{any, get}, Router};
use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};
use anyhow::{Result, Context, anyhow};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tracing::{error, info, warn};
use tracing_subscriber;
use tokio_stream::StreamExt;
use serde::Serialize;
use tower_http::services::ServeDir;

#[tracing::instrument]
async fn build_file_structure ( ) -> Result<()> {
    let data_dir = std::env::var("DATA_DIR_PATH")
        .context("'DATA_DIR_PATH' environment variable not set!")?;

    // Create the `models` directory
    info!("Building models directory...");
    let models_dir = format!("{}/models", data_dir);
    tokio::fs::create_dir_all(models_dir.clone())
        .await
        .context(format!("Failed to create directory: {models_dir}"))?;

    // Build the `logs` directory
    info!("Building logs directory...");
    let logs_dir = format!("{}/logs", data_dir);
    tokio::fs::create_dir_all(logs_dir.clone())
        .await
        .context(format!("Failed to create directory: {logs_dir}"))?;

    Ok(())
}

#[derive(Debug)]
struct OllamaServer {
    child: Child,
    stdout: ChildStdout,
    stderr: ChildStderr,
}
#[tracing::instrument]
fn start_ollama_serve ( ) -> Result<OllamaServer> {
    let data_dir = std::env::var("DATA_DIR_PATH")
        .context("'DATA_DIR_PATH' environment variable not set!")?;
    let model_dir = format!("{}/models", data_dir);

    info!("Starting Ollama serve with models directory '{model_dir}'...");
    let mut child = Command::new("ollama")
        .arg("serve")
        .env("OLLAMA_MODELS", &model_dir)
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to start Ollama serve!")?;

    let stdout = child
        .stdout
        .take()
        .context("Failed to take stdout!")?;
    let stderr = child
        .stderr
        .take()
        .context("Failed to take stderr!")?;
    
    Ok(OllamaServer {
        child,
        stdout: stdout,
        stderr: stderr,
    })
}

#[tracing::instrument]
async fn write_logs (
    ollama_server: &mut OllamaServer
) -> Result<()> {
    let now = SystemTime::now();
    let timestamp = now.duration_since(UNIX_EPOCH)
        .context("Failed to get timestamp!")?;
    let log_path = format!(
        "{}/logs/ollama-{}.log",
        std::env::var("DATA_DIR_PATH")?, 
        timestamp.as_secs());

    info!("Writing Ollama logs to {log_path}");
    let mut log_file = tokio::fs::File::create(log_path)
        .await
        .context("Failed to create log file!")?;
    tokio::io::copy(&mut ollama_server.stdout, &mut log_file)
        .await
        .context("Failed to copy stdout to log file!")?;
    tokio::io::copy(&mut ollama_server.stderr, &mut log_file)
        .await
        .context("Failed to copy stderr to log file!")?;

    Ok(())
}


#[tracing::instrument]
async fn handle_socket_helper (
    socket: WebSocket
) -> () {
    if let Err(e) = handle_socket(socket).await {
        error!("Failed to handle socket! Error: {e:?}");
    }
}

#[derive(Debug, Serialize)]
enum MessageType {
    User(String),
    DeepSeekR1(String)
}
#[derive(Debug, Serialize)]
#[serde(tag = "event")]
enum Event {
    Token { token: String },
    Thinking,
    DoneThinking,
    Done
}
#[tracing::instrument]
async fn handle_socket (
    mut socket: WebSocket
) -> Result<()> {
    let ollama = Ollama::default();
    let model = std::env::var("MODEL_NAME")
        .unwrap_or_else(|_| {
            warn!("MODEL_NAME environment variable not set, defaulting to 'deepseek-r1:8b");
            String::from("deepseek-r1:8b")
        });

    info!("Pulling model '{model}'...");
    ollama.pull_model(model.clone(), false)
        .await
        .map_err(|e| anyhow!("Failed to pull model! Error: {e:?}"))?;

    info!("Entering WS REPL...");
    let mut history: Vec<MessageType> = vec!();
    while let Some(msg) = socket.recv().await {
        info!("Taking user input...");
        let msg = if let Ok(msg) = msg {
            msg
        } else {
            return Ok(());
        };

        let prompt = match msg {
            Message::Text(text) => text,
            _ => {
                return Ok(());
            }
        }.as_str().to_string();

        if prompt.trim() == "exit" {
            break;
        }

        let history_json = serde_json::to_string(&history)
            .context("Failed to serialize history!")?;
        let augmented_prompt = format!("```CHAT_HISTORY\n{history_json}\n```\n\n\n{prompt}");
        history.push(MessageType::User(prompt.clone()));
        println!("History: {:?}", history_json);

        let mut stream = ollama
            .generate_stream(GenerationRequest::new(model.clone(), augmented_prompt))
            .await
            .map_err(|e| {
                let msg = format!("Failed to generate completion! Error: {e:?}");
                error!("{}", msg);
                anyhow!(msg)
            })?;

        let mut buffer = String::new();
        while let Some(responses) = stream
            .next()
            .await
            .and_then(|r| Some(r.map_err(|e| {
                let msg = format!("Failed to get next response! Error: {e:?}");
                anyhow!(msg)
            })))
        {
            // Unpack the response
            let responses = responses?;

            for resp in responses {
                let token = resp.response;

                // Handle the token
                if token == "<think>" {
                    eprintln!("[ Thinking... ]");
                    socket.send(
                        Message::text(serde_json::to_string(&Event::Thinking)
                            .context("Failed to serialize 'thinking' event!")?)
                    ).await
                        .map_err(|e| {
                            error!("Failed to send message to client! Error: {e:?}");
                            anyhow!("Failed to send message to client! Error: {e:?}")
                        })?;
                    continue;
                } else if token == "</think>" {
                    eprintln!("[ Done Thinking ]");
                    socket.send(
                        Message::text(serde_json::to_string(&Event::DoneThinking)
                            .context("Failed to serialize 'done thinking' event!")?)
                    ).await
                        .map_err(|e| {
                            error!("Failed to send message to client! Error: {e:?}");
                            anyhow!("Failed to send message to client! Error: {e:?}")
                        })?;
                    buffer.clear(); 
                    continue;
                }
                
                eprint!("{token}");
                buffer.push_str(&token);
                socket.send(
                    Message::text(serde_json::to_string(&Event::Token { token })
                        .context("Failed to serialize 'token' event!")?)
                ).await
                    .map_err(|e| {
                        error!("Failed to send message to client! Error: {e:?}");
                        anyhow!("Failed to send message to client! Error: {e:?}")
                    })?;
            }
        }

        eprintln!();
        history.push(MessageType::DeepSeekR1(buffer.clone()));
        socket.send(
            Message::text(serde_json::to_string(&Event::Done)
                .context("Failed to serialize 'done' event!")?)
        ).await
            .map_err(|e| {
                error!("Failed to send message to client! Error: {e:?}");
                anyhow!("Failed to send message to client! Error: {e:?}")
            })?;
    }

    Ok(())
}

async fn handler(
    ws: WebSocketUpgrade
) -> Response {
    ws.on_upgrade(move |socket| handle_socket_helper(socket))
}

#[tracing::instrument]
async fn start_webserver ( ) -> Result<()> {
    let app = Router::new()
        .route("/ws", any(handler))
        .route("/", get(|| async { Html(include_str!("../public/pages/index.html")) }))
        .nest_service("/public", ServeDir::new("public"));
    
    let port = std::env::var("PORT").unwrap_or("5776".to_string());
    eprintln!("[ Starting deepseek-r1s on {port}... ]");
    let listener = tokio::net::TcpListener::bind(&format!("0.0.0.0:{port}")).await
        .context("Couldn't start up listener!")?;
    axum::serve(listener, app).await
        .context("Could't serve the API!")?;

    Ok(())
}

#[tokio::main]
#[tracing::instrument]
async fn main() -> Result<()> {
    // Build the tracing subscriber
    tracing_subscriber::fmt::init();

    let spawn_ollama = std::env::var("SPAWN_OLLAMA")
        .unwrap_or("true".to_string())
        .parse::<bool>()
        .context("Failed to parse 'SPAWN_OLLAMA' environment variable!")?;

    let ollama_server = if spawn_ollama {
        info!("Ensuring file structure...");
        build_file_structure().await
            .context("Failed to build file structure!")?;

        info!("Starting Ollama serve...");
        Some(start_ollama_serve()
                .context("Failed to start Ollama serve!")?
        )
    } else { 
        info!("Skipping Ollama serve...");
        None
    };
    
    // Start the webserver
    let webserver = tokio::spawn(start_webserver());


    // Await stdin to kill the server
    let mut input = String::new();
    while input.trim() != "exit" {
        input.clear();
        std::io::stdin().read_line(&mut input)
            .context("Failed to read input!")?;
    }


    // Kill the server
    info!("Shutting down server...");
    webserver.abort();

    if let Some(mut ollama_server) = ollama_server {
        info!("Preparing to write logs...");
        write_logs(&mut ollama_server).await
            .context("Failed to write logs!")?;

        info!("Killing Ollama server...");
        ollama_server
            .child
            .kill()
            .await
            .context("Failed to kill Ollama server!")?;
    }

    Ok(())
}
