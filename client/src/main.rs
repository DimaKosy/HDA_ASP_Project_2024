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
    let (reader, mut writer) = (&stream, &stream);
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
                                println!("Received unknown: {}", line);
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
                    writer.write_all(line.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
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
