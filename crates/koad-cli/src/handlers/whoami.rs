use koad_core::db::KoadDB;
use anyhow::Result;
use fred::interfaces::HashesInterface;
use koad_core::config::KoadConfig;
use koad_core::session::AgentSession;
use koad_core::utils::redis::RedisClient;
use std::env;

pub async fn handle_whoami(config: &KoadConfig, _db: &KoadDB) -> Result<()> {
    let session_id = env::var("KOAD_SESSION_ID").unwrap_or_default();
    let body_id = env::var("KOAD_BODY_ID").unwrap_or_default();

    let redis_client = RedisClient::new(&config.home.to_string_lossy(), false).await?;

    // 1. Try direct SID lookup
    if !session_id.is_empty() {
        let session_key = format!("koad:session:{}", session_id);
        let res: Option<String> = redis_client.pool.hget("koad:state", &session_key).await?;
        if let Some(data) = res {
            if let Ok(session) = serde_json::from_str::<AgentSession>(&data) {
                print_session_info(&session, &session_id);
                return Ok(());
            }
        }
    }

    // 2. Try Body ID scan (Session might have been re-generated during boot)
    if !body_id.is_empty() {
        let all_state: std::collections::HashMap<String, String> =
            redis_client.pool.hgetall("koad:state").await?;
        for (key, val) in all_state {
            if key.starts_with("koad:session:") {
                if let Ok(session) = serde_json::from_str::<AgentSession>(&val) {
                    if session.body_id == body_id && session.status == "active" {
                        let sid = key.replace("koad:session:", "");
                        print_session_info(&session, &sid);
                        return Ok(());
                    }
                }
            }
        }
    }

    if !session_id.is_empty() {
        println!("\x1b[33m[Warning] KOAD_SESSION_ID is set but no matching active session was found in the Citadel.\x1b[0m");
    }

    // Fallback to active agent from environment or first identity in config
    let fallback_id = env::var("KOAD_AGENT_NAME")
        .ok()
        .and_then(|name| config.identities.get(&name.to_lowercase()))
        .or_else(|| config.identities.values().next());

    if let Some(id) = fallback_id {
        println!(
            "\x1b[33m[NOT_TETHERED]\x1b[0m (Using local config)\nIdentity: {} [{}]\nBio:      {}",
            id.name, id.role, id.bio
        );
    } else {
        println!("\x1b[33m[NOT_TETHERED]\x1b[0m No identities found in config.");
    }


    Ok(())
}

fn print_session_info(session: &AgentSession, sid: &str) {
    let bio = session
        .metadata
        .get("bio")
        .map(|b| b.to_string())
        .unwrap_or_else(|| "Active KoadOS Agent".to_string());

    println!(
        "\x1b[32m[TETHERED]\x1b[0m\nIdentity: {} [{:?}]\nRank:     {:?}\nBio:      {}\nSession:  {}\nBody:     {}",
        session.identity.name,
        session.identity.rank,
        session.identity.rank,
        bio,
        sid,
        session.body_id
    );
}
