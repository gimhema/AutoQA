mod actor;
mod agent;
mod critics;
mod game_interface;
mod llm_interface;
mod observation;
mod policy;
mod status_observer;
mod conn;
mod conn_message;
mod policy_discrete;
mod policy_continuous;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <host:port> <intent>", args[0]);
        std::process::exit(1);
    }

    let addr = &args[1];
    let intent = args[2..].join(" ");

    eprintln!("[Ouroboros] connecting to {addr} …");
    let mut agent = agent::Agent::connect(addr.as_str(), &intent)
        .expect("failed to connect to game server");

    if let Err(e) = agent.run() {
        eprintln!("[Ouroboros] error: {e}");
        std::process::exit(1);
    }
}
