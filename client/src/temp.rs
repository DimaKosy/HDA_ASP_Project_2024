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
use std::{error::Error, io};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Serialize, Deserialize, Debug)]
struct FileTransfer {
    destination: String,
    filename: String,
    content: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
enum Message {
    Text { destination: String, content: String },
    File { transfer: FileTransfer },
    System { info: String },
}

fn main() -> Result<()> {
    task::block_on(try_run("127.0.0.1:8080"))
}

async fn try_run(addr: impl ToSocketAddrs) -> Result<()> {
    let stream = TcpStream::connect(addr).await?;
    let (reader, writer) = (&stream, &stream);
    let mut lines_from_server = BufReader::new(reader).lines().fuse();
    let mut lines_from_stdin = BufReader::new(stdin()).lines().fuse();
    let mut writer = writer.clone(); // clone writer to use it mutably

    loop {
        select! {
            line = lines_from_server.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    if line.starts_with("SYS:") {
                        let system_msg = line[4..].to_string();
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
                                println!("  {}", line);
                            }
                        }
                    }
                },
                None => break,
            },
            line = lines_from_stdin.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    println!("Sending input: {}", line);

                    if line.starts_with("file:") {
                        let parts: Vec<&str> = line.splitn(2, ' ').collect();
                        if parts.len() == 2 {
                            let destination = parts[0].trim_start_matches("file:");
                            let filename = parts[1].trim();
                            send_file(destination, filename, &mut writer).await?;
                        } else {
                            println!("Invalid file command format. Use: file:<destination> <filepath>");
                        }
                    } else if line.starts_with("text:") {
                        let parts: Vec<&str> = line.splitn(2, ' ').collect();
                        if parts.len() == 2 {
                            let destination = parts[0].trim_start_matches("text:");
                            let content = parts[1].trim();
                            send_text(destination, content, &mut writer).await?;
                        } else {
                            println!("Invalid text command format. Use: text:<destination> <message>");
                        }
                    } else if line.starts_with("system:") {
                        let info = line.trim_start_matches("system:").trim();
                        send_system_message(info, &mut writer).await?;
                    } else {
                        println!("Invalid message format: {}", line);
                    }
                }
                None => break,
            }
        }
    }
    Ok(())
}

async fn send_file(destination: &str, filename: &str, writer: &mut TcpStream) -> Result<()> {
    let content = std::fs::read(filename)?;
    let transfer = FileTransfer {
        destination: destination.to_string(),
        filename: filename.to_string(),
        content,
    };
    let msg = Message::File { transfer };
    let serialized = serde_json::to_string(&msg)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

async fn send_text(destination: &str, content: &str, writer: &mut TcpStream) -> Result<()> {
    let msg = Message::Text {
        destination: destination.to_string(),
        content: content.to_string(),
    };
    let serialized = serde_json::to_string(&msg)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

async fn send_system_message(info: &str, writer: &mut TcpStream) -> io::Result<()> {
    let msg = Message::System {
        info: info.to_string(),
    };
    let serialized = serde_json::to_string(&msg)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

async fn save_to_file(transfer: &FileTransfer) -> Result<()> {
    let mut file = File::create(&transfer.filename).await?;
    file.write_all(&transfer.content).await?;
    Ok(())
}

