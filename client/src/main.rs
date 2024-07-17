#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

use async_std::{
    fs::File,
    io::{stdin, BufReader},
    net::{TcpStream, ToSocketAddrs},
    prelude::*,
    task,
};
use futures::{select, FutureExt};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Serialize, Deserialize, Debug)]
struct FileTransfer {
    destination: String,
    filename: String,
    data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum Message {
    Text { destination: String, content: String },
    File { transfer: FileTransfer },
    System { info: String },
}

// main
fn main() -> Result<()> {
    task::block_on(try_run("127.0.0.1:8080"))
}

async fn try_run(addr: impl ToSocketAddrs) -> Result<()> {
    let stream = TcpStream::connect(addr).await?;
    let (reader, writer) = (&stream, &stream);
    let mut lines_from_server = BufReader::new(reader).lines().fuse();
    let mut lines_from_stdin = BufReader::new(stdin()).lines().fuse();

    loop {
        select! {
            line = lines_from_server.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    //println!("Received: {}", line);
                    // Check for SYS: prefix
                    if line.starts_with("SYS:") {
                        let system_msg = line[4..].trim().to_string();
                        println!("System message: {}", system_msg);
                    } else {
                        match serde_json::from_str::<Message>(&line) {
                            Ok(Message::Text { destination, content }) => {
                                println!("Message to {}: {}", destination, content);
                            }
                            Ok(Message::File { transfer }) => {
                                save_to_file(&transfer).await?;
                                println!("File {} saved.", transfer.filename);
                            }
                            Ok(Message::System { info }) => {
                                println!("System message: {}", info);
                            }
                            Err(e) => {
                                println!("Received unknown message: {}", line);
                                println!("Error: {}", e);
                            }
                        }
                    }
                },
                None => break,
            },
            line = lines_from_stdin.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    let (dest, msg_type) = match line.find(":") {
                        None => continue,
                        Some(idx) => (&line[..idx], line[idx + 1 ..].trim()),
                    };
                    match msg_type {
                        msg if msg.starts_with("file:") => {
                            let filename = msg.trim_start_matches("file:").trim();
                            send_file(dest, filename, &mut writer.clone()).await?;
                        }
                        msg if msg.starts_with("text:") => {
                            let content = msg.trim_start_matches("text:").trim();
                            send_text(dest, content, &mut writer.clone()).await?;
                        }
                        msg if msg.starts_with("system:") => {
                            let info = msg.trim_start_matches("system:").trim();
                            send_system_message(dest, info, &mut writer.clone()).await?;
                        }
                        _ => {
                            println!("Invalid message format: {}", line);
                        }
                    }
                }
                None => break,
            }
        }
    }
    Ok(())
}

async fn send_file(destination: &str, filename: &str, writer: &mut TcpStream) -> Result<()> {
    let mut file = File::open(filename).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;

    let file_transfer = FileTransfer {
        destination: destination.to_string(),
        filename: filename.to_string(),
        data: buffer,
    };

    let message = Message::File { transfer: file_transfer };
    let serialized = serde_json::to_string(&message)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    println!("File {} sent to {}.", filename, destination);
    Ok(())
}

async fn send_text(destination: &str, content: &str, writer: &mut TcpStream) -> Result<()> {
    let message = Message::Text { destination: destination.to_string(), content: content.to_string() };
    let serialized = serde_json::to_string(&message)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    println!("Text sent to {}: {}", destination, content);
    Ok(())
}

async fn send_system_message(destination: &str, info: &str, writer: &mut TcpStream) -> Result<()> {
    let message = Message::System { info: info.to_string() };
    let serialized = serde_json::to_string(&message)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    println!("System message sent to {}: {}", destination, info);
    Ok(())
}

async fn save_to_file(file_transfer: &FileTransfer) -> Result<()> {
    let mut file = File::create(&file_transfer.filename).await?;
    file.write_all(&file_transfer.data).await?;
    Ok(())
}
