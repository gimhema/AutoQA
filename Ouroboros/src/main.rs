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
    let mut action_space_json: Option<String> = None;
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
            "--action-space" => {
                i += 1;
                action_space_json = Some(args.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("--action-space requires a JSON value");
                    std::process::exit(1);
                }));
            }
            _ => positional.push(args[i].clone()),
        }
        i += 1;
    }

    if positional.len() < 2 {
        eprintln!(
            "Usage: {} <host:port> <intent> [options]\n\n\
             Options:\n  \
               --llm-endpoint URL    LLM server endpoint (default: {})\n  \
               --llm-model NAME      model name (default: {})\n  \
               --action-space JSON   action space as JSON array (e.g. '[\"jump\",\"fire\"]')",
            args[0], DEFAULT_LLM_ENDPOINT, DEFAULT_LLM_MODEL
        );
        std::process::exit(1);
    }

    let addr = &positional[0];
    let intent = positional[1..].join(" ");

    eprintln!("[Ouroboros] connecting to {addr} …");
    let mut agent = agent::Agent::connect(addr.as_str(), &intent)
        .expect("failed to connect to game server");

    if let Some(json_str) = action_space_json {
        let actions: Vec<serde_json::Value> = serde_json::from_str(&json_str)
            .unwrap_or_else(|e| {
                eprintln!("--action-space: invalid JSON: {e}");
                std::process::exit(1);
            });
        agent.set_action_space(policy_gen::ActionSpace::Discrete {
            available_actions: actions,
        });
    }

    let llm = llm_interface::LlmClient::new(llm_endpoint.clone(), llm_model.clone());
    agent.set_llm(llm);
    eprintln!("[Ouroboros] LLM: {llm_model} @ {llm_endpoint}");

    if let Err(e) = agent.run() {
        eprintln!("[Ouroboros] error: {e}");
        std::process::exit(1);
    }
}
