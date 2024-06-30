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

// main
fn main() -> Result<()> {
    task::block_on(try_run("127.0.0.1:8080"))
}

async fn try_run(addr: impl ToSocketAddrs) -> Result<()> {
    let stream = TcpStream::connect(addr).await?;
    let (reader, mut writer) = (&stream, &stream); // 1
    let mut lines_from_server = BufReader::new(reader).lines().fuse(); // 2
    let mut lines_from_stdin = BufReader::new(stdin()).lines().fuse(); // 2
    
    loop {
        select! { // 3
            line = lines_from_server.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    println!("{}", line);

                    // Deserialize if it's a file transfer
                    if let Ok(file_transfer) = serde_json::from_str::<FileTransfer>(&line) {
                        save_to_file(&file_transfer).await?;
                        println!("File {} saved.", file_transfer.filename);
                    }
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
                    if msg_type.starts_with("file:") {
                        let filename = msg_type.trim_start_matches("file:").trim();
                        send_file(dest, filename, &mut writer.clone()).await?;
                    } else if msg_type.starts_with("text:") {
                        writer.write_all(line.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
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

    let serialized = serde_json::to_string(&file_transfer)?;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    println!("File {} sent to {}.", filename, destination);
    Ok(())
}

async fn save_to_file(file_transfer: &FileTransfer) -> Result<()> {
    let mut file = File::create(&file_transfer.filename).await?;
    file.write_all(&file_transfer.data).await?;
    Ok(())
}