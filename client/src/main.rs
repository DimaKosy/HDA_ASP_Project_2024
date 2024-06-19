// A command line chat client will probably need some styling / high-level interaction with the command line. So this
// project includes `crossterm` by default, a Rust library for cross-plattform command line manipulation

// use crossterm::{
//     style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
//     ExecutableCommand,
// };

// fn main() {
//     // crossterm commands can fail. For now, you are allowed to use `.unwrap()`, but you can also take a look at error
//     // handling in Rust and do it right from the start. We will talk about error handling probably towards the end of
//     // May

//     std::io::stdout()
//         .execute(SetForegroundColor(Color::Blue))
//         .unwrap()
//         .execute(SetBackgroundColor(Color::Red))
//         .unwrap()
//         .execute(Print("Styled text here."))
//         .unwrap()
//         .execute(ResetColor)
//         .unwrap();
// }

use async_std::{
    io::{stdin, BufReader},
    net::{TcpStream, ToSocketAddrs},
    prelude::*,
    task,
};
use futures::{select, FutureExt};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
                },
                None => break,
            },
            line = lines_from_stdin.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    writer.write_all(line.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                }
                None => break,
            }
        }
    }
    Ok(())
}
