use std::collections::HashMap;
use std::error::Error;
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

struct Client {
    fd: usize,
    nick: String,
}

struct ChatState {
    clients: Arc<Mutex<HashMap<usize, Client>>>,
}

impl ChatState {
    async fn new() -> Result<Self> {
        Ok(Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn add_client(&self, fd: usize, client: Client) {
        let mut clients = self.clients.lock().unwrap();
        clients.insert(fd, client);
    }

    async fn remove_client(&self, fd: usize) {
        let mut clients = self.clients.lock().unwrap();
        clients.remove(&fd);
    }

    async fn broadcast(&self, sender_fd: usize, message: String) -> Result<()> {
        let clients = self.clients.lock().unwrap();
        for (fd, client) in clients.iter() {
            if *fd != sender_fd {
                client.send_message(message.clone()).await?;
            }
        }
        Ok(())
    }
}

impl Client {
    async fn new(fd: usize) -> Result<Self> {
        Ok(Self {
            fd,
            nick: format!("user:{}", fd),
        })
    }

    async fn send_message(&self, message: String) -> Result<()> {
        // 在这里实现发送消息的逻辑
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let chat_state = ChatState::new().await?;
    let listener = TcpListener::bind("0.0.0.0:3333").await?;

    loop {
        let (mut stream, _) = listener.accept().await?;
        let fd = stream.as_raw_fd() as usize;
        let client = Client::new(fd).await?;
        chat_state.add_client(fd, client).await;

        let welcome_msg = "Welcome to Simple Chat! Use /nick <nick> to set your nick.\n";
        stream.write(welcome_msg.as_bytes()).await?;

        println!("Connected client fd={}", fd);

        tokio::spawn(handle_connection(chat_state.clone(), stream));
    }
}

async fn handle_connection(chat_state: Arc<ChatState>, mut stream: TcpStream) -> Result<()> {
    let fd = stream.as_raw_fd() as usize;
    let mut buffer = [0; 256];

    loop {
        let n = stream.read(&mut buffer).await?;

        if n == 0 {
            println!("Disconnected client fd={}, nick={}", fd, chat_state.clients.lock().unwrap()[&fd].nick);
            chat_state.remove_client(fd).await;
            break;
        }

        let message = String::from_utf8_lossy(&buffer[..n]).to_string();

        if message.starts_with('/') {
            let parts: Vec<&str> = message.split(' ').collect();
            let command = parts[0];
            let arg = parts.get(1).cloned().unwrap_or("");

            if command == "/nick" && !arg.is_empty() {
                let mut clients = chat_state.clients.lock().unwrap();
                let client = clients.get_mut(&fd).unwrap();
                client.nick = arg.to_string();
            } else {
                let errmsg = "Unsupported command\n";
                stream.write(errmsg.as_bytes()).await?;
            }
        } else {
            let clients = chat_state.clients.lock().unwrap();
            let nick = &clients[&fd].nick;
            let msg = format!("{}> {}", nick, message);

            println!("{}", msg);
            chat_state.broadcast(fd, msg).await?;
        }
    }

    Ok(())
}


