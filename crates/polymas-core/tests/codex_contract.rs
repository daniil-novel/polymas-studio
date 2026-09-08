use polymas_core::codex::{CodexRequest, CodexStream, JsonlDecoder, build_arguments};
use polymas_core::registry::Sandbox;
use serde_json::json;

fn request(session: Option<String>) -> CodexRequest {
    CodexRequest {
        workspace: std::env::temp_dir().join("Agent Work/one"),
        session_id: session,
        model: None,
        sandbox: Sandbox::ReadOnly,
        prompt: "literal $(secret) `command` \" ; text".into(),
        ..Default::default()
    }
}
#[test]
fn prompt_is_never_interpolated_in_shell_or_process_arguments() {
    let r = request(None);
    let args = build_arguments(&r).unwrap();
    assert!(!args.iter().any(|a| a.contains("secret")));
    assert_eq!(args.last().unwrap(), "-");
    assert!(args.iter().any(|a| a == "--ignore-user-config"));
    assert!(args.iter().any(|a| a == "features.multi_agent=false"));
    assert!(!args.iter().any(|a| a.contains("bypass")));
}
#[test]
fn resume_uses_exact_uuid_and_never_last_or_unsupported_cd_flag() {
    let session = "0199a213-81c0-7800-8aa1-bbab2a035a53";
    let args = build_arguments(&request(Some(session.into()))).unwrap();
    assert_eq!(&args[..2], &["exec", "resume"]);
    assert!(args.contains(&session.into()));
    assert!(!args.contains(&"--last".into()) && !args.contains(&"--cd".into()));
    assert!(build_arguments(&request(Some("--last".into()))).is_err());
}
#[test]
fn partial_utf8_and_partial_lines_are_decoded_without_losing_identity() {
    let bytes =
        "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Привет\"}}\n"
            .as_bytes();
    let mut decoder = JsonlDecoder::default();
    let mut events = vec![];
    for byte in bytes {
        events.extend(decoder.push(&[*byte]).unwrap());
    }
    decoder.finish().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["item"]["text"], "Привет");
}
#[test]
fn malformed_or_oversized_stream_never_becomes_success() {
    let mut decoder = JsonlDecoder::default();
    assert!(decoder.push(b"not json\n").is_err());
    let mut decoder = JsonlDecoder::default();
    assert!(decoder.push(&vec![b'x'; 1_048_577]).is_err());
    let mut decoder = JsonlDecoder::default();
    decoder.push(b"{\"type\":").unwrap();
    assert!(decoder.finish().is_err());
}

#[test]
fn final_text_without_turn_completed_is_not_a_successful_turn() {
    let mut stream = CodexStream::new(None);
    stream
        .observe(
            &json!({"type":"thread.started","thread_id":"0199a213-81c0-7800-8aa1-bbab2a035a53"}),
        )
        .unwrap();
    stream
        .observe(&json!({"type":"item.completed","item":{"type":"agent_message","text":"done"}}))
        .unwrap();
    assert!(stream.finish().is_err());
}

#[test]
fn cross_agent_session_substitution_is_rejected() {
    let mut stream = CodexStream::new(Some("0199a213-81c0-7800-8aa1-bbab2a035a53".into()));
    assert!(
        stream
            .observe(
                &json!({"type":"thread.started","thread_id":"0199a213-81c0-7800-8aa1-bbab2a035a54"})
            )
            .is_err()
    );
}
