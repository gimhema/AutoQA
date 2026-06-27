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
mod policy_gen;

const DEFAULT_LLM_ENDPOINT: &str = "http://localhost:11434/v1/chat/completions";
const DEFAULT_LLM_MODEL: &str = "llama3.2:1b";

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut llm_endpoint = DEFAULT_LLM_ENDPOINT.to_string();
    let mut llm_model = DEFAULT_LLM_MODEL.to_string();
    let mut positional = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--llm-endpoint" => {
                i += 1;
                llm_endpoint = args.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("--llm-endpoint requires a value");
                    std::process::exit(1);
                });
            }
            "--llm-model" => {
                i += 1;
                llm_model = args.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("--llm-model requires a value");
                    std::process::exit(1);
                });
            }
            _ => positional.push(args[i].clone()),
        }
        i += 1;
    }

    if positional.len() < 2 {
        eprintln!(
            "Usage: {} <host:port> <intent> [--llm-endpoint URL] [--llm-model NAME]",
            args[0]
        );
        std::process::exit(1);
    }

    let addr = &positional[0];
    let intent = positional[1..].join(" ");

    eprintln!("[Ouroboros] connecting to {addr} …");
    let mut agent = agent::Agent::connect(addr.as_str(), &intent)
        .expect("failed to connect to game server");

    let llm = llm_interface::LlmClient::new(llm_endpoint.clone(), llm_model.clone());
    agent.set_llm(llm);
    eprintln!("[Ouroboros] LLM: {llm_model} @ {llm_endpoint}");

    if let Err(e) = agent.run() {
        eprintln!("[Ouroboros] error: {e}");
        std::process::exit(1);
    }
}
