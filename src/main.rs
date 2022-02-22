extern crate core;

use futures::FutureExt;
use std::error::Error;
use std::io::{self, Write};
use std::io::{ErrorKind, Read};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::{thread, time};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

// todo for gui: handle relative path warnings in torrc

mod config;
mod error;
mod init;

type TStdOutData = Arc<Mutex<Vec<u8>>>;

fn stdout_stream_to_vec<R>(mut stream: R) -> TStdOutData
where
    R: Read + Send + 'static,
{
    let out = Arc::new(Mutex::new(Vec::new()));
    let vec = out.clone();
    thread::Builder::new()
        .name("stdout_stream_to_vec".into())
        .spawn(move || loop {
            let mut buf = [0];
            match stream.read(&mut buf) {
                Err(err) => {
                    log::info!("{}] Error reading from stream: {}", line!(), err);
                    break;
                }
                Ok(got) => match got {
                    0 => break,
                    1 => vec.lock().expect("!lock").push(buf[0]),
                    _ => {
                        log::info!("{}] Unexpected number of bytes: {}", line!(), got);
                        break;
                    }
                },
            }
        })
        .expect("!thread");
    out
}

async fn transfer(mut inbound: TcpStream, proxy_addr: String) -> Result<(), Box<dyn Error>> {
    let mut outbound = TcpStream::connect(proxy_addr).await?;

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = async {
        tokio::io::copy(&mut ri, &mut wo).await?;
        wo.shutdown().await
    };

    let server_to_client = async {
        tokio::io::copy(&mut ro, &mut wi).await?;
        wi.shutdown().await
    };

    tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}

async fn main_impl() -> Result<(), Box<dyn Error>> {
    let the_config = init::init()?;
    let ports: Vec<u16> = (the_config.tor.start_port
        ..the_config.tor.start_port + the_config.tor.port_count)
        .collect();
    let tor_addrs = ports
        .iter()
        .map(|x| "127.0.0.1:".to_string() + &x.to_string())
        .collect::<Vec<String>>();
    let out_list: Result<Vec<TStdOutData>, error::TorSpawnError> = ports
        .iter()
        .map(|x| {
            let res = Command::new(&the_config.tor.full_path)
                .args(&[
                    "-f",
                    &the_config.tor.torrc_full_path,
                    "--SocksPort",
                    &x.to_string(),
                    "--DataDirectory",
                    &(the_config.tor.data_dirs.full_path.clone() + &x.to_string()),
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            match res {
                Ok(mut child) => Ok(stdout_stream_to_vec(child.stdout.take().expect("!stdout."))),
                Err(e) => Err(match e.kind() {
                    ErrorKind::NotFound => error::TorSpawnError::NotFound {
                        path: the_config.tor.full_path.clone(),
                    },
                    _ => error::TorSpawnError::Other {
                        path: the_config.tor.full_path.clone(),
                        error: e.to_string(),
                    },
                }),
            }
        })
        .collect();

    match out_list {
        Ok(out_list) => {
            tokio::spawn(async move {
                loop {
                    for (i, out) in out_list.iter().enumerate() {
                        loop {
                            let mut vec_guard = out.lock().unwrap();
                            let s = String::from_utf8((*vec_guard).clone());
                            if let Ok(ss) = s {
                                if ss.is_empty() {
                                    break;
                                }
                                log::info!("{i}: {ss}");
                                io::stdout().flush().unwrap();
                                (*vec_guard).clear();
                            }
                        }
                    }
                    tokio::time::sleep(time::Duration::from_millis(50)).await;
                }
            });

            let listen_addr = the_config.listen_addr.clone();
            log::info!("Listening on: {}", listen_addr);
            let listener = TcpListener::bind(listen_addr).await?;
            let idx_mutex = Mutex::new(0u16);

            loop {
                match listener.accept().await {
                    Ok((inbound, _)) => {
                        let mut idx_guard = idx_mutex.lock().unwrap();
                        if *idx_guard > the_config.tor.port_count - 1 {
                            *idx_guard = 0;
                        }
                        let server_addr = tor_addrs[*idx_guard as usize].clone();
                        *idx_guard += 1;
                        std::mem::drop(idx_guard);

                        let transfer = transfer(inbound, server_addr.clone()).map(|r| {
                            if let Err(_e) = r {
                                //                        println!("Failed to transfer; error={}", e);
                                //                        io::stdout().flush().unwrap();
                            }
                        });

                        tokio::spawn(transfer);
                    }
                    // https://stackoverflow.com/questions/2569620/socket-accept-error-24-to-many-open-files
                    Err(e) => log::info!("couldn't get client: {:?}", e),
                }
            }
        }
        Err(e) => Err(e.into()),
    }
}

#[tokio::main]
async fn main() {
//    config::save_default_config().unwrap();
    if let Err(e) = &main_impl().await {
        log::error!("{e}");
        println!("{e}");
        std::process::exit(1);
    }
}
