// Some thoughts on the chat server:
// - This is a server, so an executable that runs perpetually! So there will be a loop, maybe? What will that loop do?
// - At some point, you want to configure your server: Where should it run? Maybe limit the number of concurrent users? What else would you like to configure? How would you do the configuring?
// - Users should be able to message each other. What types of messaging do you want to support? Only one-on-one or also rooms/groups etc? How will the messages look like? Should users be able to send each other files?
// - What job does the server have when it comes to messages? Does it only facilitate peer-to-peer communication between clients, or do all messages go through the server?
//   - What would be the benefits and drawbacks of each approach?
// - Do you want/need some form of user management? If so, how would that look like?


use async_std::net::TcpStream;

use futures::channel::mpsc;
use futures::select;
use futures::FutureExt;
use futures::sink::SinkExt;
use std::sync::Arc;
use std::collections::hash_map::{Entry, HashMap};

// Boiler plate
use async_std::{
    io::BufReader,  
    prelude::*,
    task, 
    net::{TcpListener, ToSocketAddrs}, 
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

//Event Queue
enum Event { // 1
    NewPeer {
        name: String,
        stream: Arc<TcpStream>,
        shutdown: Receiver<Void>,
    },
    Message {
        from: String,
        to: Vec<String>,
        msg: String,
    },
    SysMessage {
        stream: Arc<TcpStream>,
        msg: String
    }
}

enum Void {} //Enforcer to ensure messages are sent down an uninhabited  channel

//Accept loop for incoming connections
async fn accept_loop(addr: impl ToSocketAddrs) -> Result<()> {

    //binds listener to address
    let listener = TcpListener::bind(addr).await?;

    //create broker to handle events
    let (broker_sender, broker_receiver) = mpsc::unbounded(); 
    let _broker_handle = task::spawn(broker_loop(broker_receiver)); 

    //handle listener
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;

        //Connected
        println!("Accepting from  : {}", stream.peer_addr()?);
        spawn_and_log_error(connection_loop(broker_sender.clone(), stream));
    }
    drop(broker_sender);    //closes broker so that channel is empty
    match _broker_handle.await{  //Joins broker, ensuring complition ##ASK
        Ok(()) => Ok(()),
        Err(e) => { eprintln!("{}",e);
                    panic!()},
    }
}

//Helper function for error handling
fn spawn_and_log_error<F>(fut: F) -> task::JoinHandle<()>
where
    F: Future<Output = Result<()>> + Send + 'static,
{
    task::spawn(async move {
        if let Err(e) = fut.await {
            //logs error
            eprintln!("{}", e)
        }
    })
}

async fn connection_loop(mut broker: Sender<Event>, stream: TcpStream) -> Result<()> {

    let stream = Arc::new(stream);
    let reader = BufReader::new(&*stream);
    

    broker.send(Event::SysMessage { stream: (Arc::clone(&stream)), msg: ("Please enter a username:\n\r".to_string()) })
    .await?;
    println!("Sent Welcome?");

    let mut lines = reader.lines();
    let mut name;
    loop{
        name = (match lines.next().await { 
            None => Err("peer disconnected immediately")?,
            Some(line) => line?,
        }).trim().to_ascii_lowercase().to_string();

        if (name.chars().all(char::is_alphanumeric)) {
            break;
        }
        broker.send(Event::SysMessage { stream: (Arc::clone(&stream)), msg: ("Username must only contain alpha-numeric characters\n\r".to_string()) })
        .await?;
    }
    
    
    let (_shutdown_sender, shutdown_receiver) = mpsc::unbounded::<Void>(); //only purpose is to get dropped
    //handle new connection
    broker.send(
        Event::NewPeer {
            name: name.clone(), stream: Arc::clone(&stream),shutdown: shutdown_receiver
        })
    .await?;
    
    broker.send(
        Event::SysMessage { 
            stream: (Arc::clone(&stream)), msg: (format!("Welcome {}\n\r",name))
        })
    .await?;

    while let Some(line) = lines.next().await {
        
        let line = line?;
        let (dest, msg) = match line.find(':') { //splits message between destionation and message
            None => continue,
            Some(idx) => (&line[..idx], line[idx + 1 ..].trim()),
        };
        let dest: Vec<String> = dest.split(',').map(|name| name.trim().to_string()).collect();
        let msg: String = msg.to_string();
        
        //sends messgage
        broker.send(Event::Message {
            from: name.clone(),
            to: dest,
            msg,
        }).await?;
    }
    Ok(())
}

async fn connection_writer_loop(messages: &mut Receiver<String>, stream: Arc<TcpStream>, shutdown: Receiver<Void>,) -> Result<()> {
    let mut stream = &*stream;
    let mut messages = messages.fuse();
    let mut shutdown = shutdown.fuse();

    loop { 
        select! {
            msg = messages.next().fuse() => match msg {
                Some(msg) => stream.write_all(msg.as_bytes()).await?,
                None => break,
            },
            void = shutdown.next().fuse() => match void {
                Some(void) => match void {},
                None => break,
            }
        }
    }
    Ok(())

}

async fn broker_loop(events: Receiver<Event>) -> Result<()>{
    let (disconnect_sender, mut disconnect_receiver) = mpsc::unbounded::<(String, Receiver<String>)>();
    let mut peers: HashMap<String, Sender<String>> = HashMap::new();
    let mut events = events.fuse();
    
    //#? Create new event to handle files and other data types

    //while event exists, we match the event and run it
    loop {
        let event = select! {
            event = events.next().fuse() => match event {
                None => break,
                Some(event) => event,
            },
            disconnect = disconnect_receiver.next().fuse() => {
               
                // match disconnect{
                //     Some(disconnect) => (disconnect),
                //     None => continue,
                // };
                // let (name, _pending_messages) = disconnect;
                let (name, _pending_messages) = disconnect.unwrap(); //##ASK Option -> Result
                assert!(peers.remove(&name).is_some());
                continue;
            },
        };

        match event {
            //sending message to each?? destination
            Event::Message { from, to, msg } => {
                for addr in to {
                    if let Some(peer) = peers.get_mut(&addr) {
                        let msg = format!("from {}: {}\n\r", from, msg);
                        match peer.send(msg).await{
                            Ok(_) => (),
                            Err(why) => print!("{}", why),
                        }
                    }
                }
            }
            Event::SysMessage {stream, msg } => {
                let mut stream = &*stream;
                let msg = format!("{}\n\r", msg);
                // match stream.write_all(msg.as_bytes()).await{ //##ASK "?"" not applic?
                //     Ok(_) => (),
                //     Err(why) => println!("{}",why),
                // }

                stream.write_all(msg.as_bytes()).await?;
            }
            //adding new peer
            Event::NewPeer { name, stream, shutdown } => {
                match peers.entry(name.clone()) {
                    Entry::Occupied(..) => (),
                    Entry::Vacant(entry) => {
                        let (client_sender, mut client_receiver) = mpsc::unbounded();
                        //register new peer in hashmap
                        entry.insert(client_sender); 
                        let mut disconnect_sender = disconnect_sender.clone();

                        spawn_and_log_error(async move {
                            let res = connection_writer_loop(&mut client_receiver, stream, shutdown).await;
                            disconnect_sender.send((name, client_receiver))
                            .await?;// sending peer name
                            res
                        });

                    }
                }
            }
        }
    }
    drop(peers);    //drops peer map
    drop(disconnect_sender); //drop disconnections channel
    while let Some((_name, _pending_messages)) = disconnect_receiver.next().await {
    }
    Ok(())
    

}
   
fn main() -> Result<()>{
    task::block_on(accept_loop("127.0.0.1:8080"))
}