#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

// use std::{thread, time::{self, Duration}};

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
enum Message {
    Text { content: String },
    File { transfer: FileTransfer },
    System { info: String },
}

// main
fn main() -> Result<()> {
    // for i in 1..10000{
    //     task::spawn(try_run("127.0.0.1:8080", format!("N{}\n",i)));
    //     thread::sleep(time::Duration::from_millis(30));
    //     println!("{}",i);
    // }
    
    task::block_on(try_run("127.0.0.1:8080", format!("N{}\n",0)))
}

async fn try_run(addr: impl ToSocketAddrs, idx:String) -> Result<()> {
    let stream = TcpStream::connect(addr).await?;
    let (reader, mut writer) = (&stream, &stream); // 1
    let mut lines_from_server = BufReader::new(reader).lines().fuse(); // 2
    let mut lines_from_stdin = BufReader::new(stdin()).lines().fuse(); // 2
    
    loop {
        select! { // 3
            line = lines_from_server.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    match serde_json::from_str::<Message>(&line) {
                        Ok(Message::Text { content }) => {
                            println!("Text message: {}", content);
                        }
                        Ok(Message::File { transfer }) => {
                            save_to_file(&transfer).await?;
                            println!("File {} saved.", transfer.filename);
                        }
                        Ok(Message::System { info }) => {
                            println!("System message: {}", info);
                        }
                        Err(_) => {
                            println!("Received unknown message: {}", line);
                        }
                    }
                    writer.write_all(idx.to_string().as_bytes()).await?;
                    println!("{}", idx);
                },
                None => break,
            },
            line = lines_from_stdin.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    let (dest, msg_type) = match line.find(":"){
                        None => continue,
                        Some(idx) => (&line[..idx], line[idx + 1 ..].trim()),
                    };
                    match msg_type {
                        msg if msg.starts_with("file:") => {
                            let filename = msg.trim_start_matches("file:").trim();
                            send_file(dest, &filename, &mut writer.clone()).await?;
                        }
                        msg if msg.starts_with("text:") => {
                            let text = msg.trim_start_matches("text:").trim();
                            send_text(dest, text, &mut writer.clone()).await?;
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

    // let message = Message::File {
    //     transfer: file_transfer,
    // };

    let serialized = serde_json::to_string(&file_transfer)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    println!("File {} sent to {}.", filename, destination);
    Ok(())
}

async fn send_text(destination: &str, text: &str, writer: &mut TcpStream) -> Result<()> {
    let message = Message::Text {
        content: text.to_string(),
    };

    let serialized = serde_json::to_string(&message)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    println!("Text message sent to {}.", destination);
    Ok(())
}

async fn send_system_message(destination: &str, info: &str, writer: &mut TcpStream) -> Result<()> {
    let message = Message::System {
        info: info.to_string(),
    };

    let serialized = serde_json::to_string(&message)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    println!("System message sent to {}: {}", destination, info);
    Ok(())
}

async fn save_to_file(file_transfer: &FileTransfer) -> Result<()> {
    let mut file = File::create(&file_transfer.filename).await?;
    file.write_all(&file_transfer.data).await?;
    Ok(())
}