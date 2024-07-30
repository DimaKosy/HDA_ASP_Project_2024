// imports necessary external crates and modules 
//for serializing and deserializing Rust data structures to/from JSON.
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

use std::sync::Arc;

use async_std::{
    fs::File,
    io::{stdin, BufReader},
    net::{TcpStream, ToSocketAddrs},
    prelude::*,
    task,
};
use futures::{select, FutureExt};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

//defineing structures for messages and file transfers:
#[derive(Serialize, Deserialize, Debug)]
struct FileTransfer {
    destination: String,
    filename: String,
    data: Vec<u8>,
}//Represents a file transfer with a destination, filename, and data

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum Message {
    Text {
        destination: String,
        content: String,
    },
    File {
        transfer: FileTransfer,
    },
    System {
        info: String,
    },
}// An enum that can be either a text message, a file transfer, or a system message.

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
                let (dest, msg_block) = match line.find(':') { //splits message between destination and message
                    None => {
                        println!("{}", line);
                        continue
                    },
                    Some(idx) => (&line[..idx], line[idx + 1 ..].trim()),
                };
                let (msg_type, msg) = match msg_block.find(':') {
                    None => {
                        println!("{}", msg_block);
                        continue
                    },
                    Some(idx) => (&msg_block[..idx], msg_block[idx + 1 ..].trim()),
                };
                if msg_type.eq("file") {
                    // Attempt to deserialize the message as a `Message::File`
                    let message: std::result::Result<Message, serde_json::Error> = serde_json::from_str(&line);
                    match message {
                        Ok(Message::File { transfer }) => {
                            // Print to confirm message deserialization
                            println!("Received file transfer message: {:?}", transfer);
                            save_to_file(&transfer).await?;
                            println!("File received from {}: {}", transfer.destination, transfer.filename);
                        },
                        Ok(_) => {
                            println!("Unexpected message type received");
                        },
                        Err(err) => {
                            println!("Failed to deserialize file message: {}", err);
                        }
                    }
                }
                 else if msg_type.eq("text") {
                    println!("From {}: {}", dest, msg);
                } else {
                    println!("NOT FILE");
                }
            },
            None => break,
        },
        line = lines_from_stdin.next().fuse() => match line {
            Some(line) => {
                let line = line?;
                println!("Sending input: {}", line);
                let (dest, msg_block) = match line.find(':') { //splits message between destination and message
                    None => {
                        writer.write_all(line.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                        continue
                    },
                    Some(idx) => (&line[..idx], line[idx + 1 ..].trim()),
                };
                let (msg_type, msg) = match msg_block.find(':') {
                    None => {
                        println!("FAILED");
                        continue
                    },
                    Some(idx) => (&msg_block[..idx], msg_block[idx + 1 ..].trim()),
                };
                if msg_type.eq("file") {
                    send_file(dest, msg, &stream).await?
                } else {
                    writer.write_all(line.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                }
                    // println!("\n\n{}:{}:{}\n\n",dest,msg_type,msg);

                }
                None => break,
            }
        }
    }
    Ok(())
}

async fn send_file(destination: &str, filename: &str, stream: &TcpStream) -> Result<()> {
    let mut writer = stream;
    let mut file = File::open(filename).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;

    let file_transfer = FileTransfer {
        destination: destination.to_string(),
        filename: filename.to_string(),
        data: buffer,
    };

    let message = Message::File {
        transfer: file_transfer,
    };
    let serialized = serde_json::to_string(&message)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    println!("File {} sent to {}.", filename, destination);
    Ok(())
}

async fn save_to_file(file_transfer: &FileTransfer) -> Result<()> {
    use std::path::Path;

    // Create the full path for the file
    let path = Path::new(&file_transfer.filename);

    // Create the directory if it doesn't exist
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            async_std::fs::create_dir_all(parent).await?;
        }
    }

    let mut file = File::create(path).await?;
    file.write_all(&file_transfer.data).await?;
    println!("File saved to {}", path.display());
    Ok(())
}


//send file format: user:file:/path/to/file.txt
