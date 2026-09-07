use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tau_proto::{
    CborValue, Configure, Event, HarnessInputMessage, HarnessInputReader, HarnessOutputMessage,
    HarnessOutputWriter, PromptOriginator, ToolName, ToolStarted,
};

/// Integration test that calls the actual SearXNG instance at localhost:9000.
/// Requires a running SearXNG instance.
#[test]
fn test_search_integration() {
    let config = json!({
        "base_url": "http://localhost:9000",
        "default_categories": ["general", "local"]
    });

    let configure_message = HarnessOutputMessage::Configure(Configure {
        tool_prefix: None,
        config: tau_proto::json_to_cbor(&config),
        instance_name: tau_proto::ExtensionName::parse("tau-ext-searxng-search").unwrap(),
        state_dir: None,
        secrets: BTreeMap::new(),
        settings_files: Default::default(),
    });

    let tool_started_message = HarnessOutputMessage::deliver(Event::ToolStarted(
        ToolStarted {
            invocation_policy: tau_proto::ToolInvocationPolicy::default(),
            call_id: "call-1".into(),
            tool_name: ToolName::new("searxng_search"),
            arguments: cbor_args(),
            agent_id: tau_proto::AgentId::parse("agent-1").unwrap(),
            originator: PromptOriginator::User,
        },
    ));

    let input_messages = vec![configure_message, tool_started_message];

    let (_state, frames) = run_messages(input_messages);

    let tool_result_event = frames
        .iter()
        .find_map(|f| {
            if let HarnessInputMessage::Emit(e) = f {
                let event = (*e.event).clone();
                if matches!(event, Event::ToolResultReported(_)) {
                    Some(event)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("expected Emit frame with ToolResultReported");

    let tool_result = match tool_result_event {
        Event::ToolResultReported(r) => r,
        _ => panic!("expected ToolResultReported event, got: {:?}", tool_result_event),
    };

    let output = match tool_result.result {
        CborValue::Text(s) => s,
        _ => panic!("tool result must be text"),
    };

    let parsed: serde_json::Value = serde_json::from_str(&output)
        .unwrap_or_else(|_| panic!("output must be valid JSON: {output}"));

    let results = parsed.get("results").unwrap();
    let results_array = results.as_array().unwrap_or_else(|| {
        panic!("results must be array, got: {results:?}");
    });

    assert!(
        !results_array.is_empty(),
        "search results must be non-empty: {output:?}"
    );

    let first_result = &results_array[0];
    assert!(first_result.get("title").is_some(), "result must have title");
    assert!(first_result.get("url").is_some(), "result must have url");

    println!("search succeeded with {} results", results_array.len());
}

fn cbor_args() -> CborValue {
    tau_proto::json_to_cbor(&json!({
        "query": "tau agent"
    }))
}

fn configure_message() -> HarnessOutputMessage {
    HarnessOutputMessage::Configure(Configure {
        tool_prefix: None,
        config: tau_proto::json_to_cbor(&json!({ "value": 3 })),
        instance_name: tau_proto::ExtensionName::parse("test-extension")
            .expect("test extension name must satisfy the identifier grammar"),
        state_dir: None,
        secrets: BTreeMap::new(),
        settings_files: Default::default(),
    })
}

fn run_messages(
    input: Vec<HarnessOutputMessage>,
) -> (tau_ext_searxng_search::SearXNGState, Vec<HarnessInputMessage>) {
    let mut input_bytes = Vec::new();
    let mut input_writer = HarnessOutputWriter::new(&mut input_bytes);
    if !matches!(input.first(), Some(&HarnessOutputMessage::Configure(_))) {
        input_writer
            .write_message(&configure_message())
            .expect("write initial configure");
    }
    for message in input {
        input_writer.write_message(&message).expect("write input");
    }
    input_writer.flush().expect("flush input");

    let writer = SharedWriter::default();
    let written = writer.clone();
    let state = tau_client::TauExtensionRunner::new(tau_ext_searxng_search::SearXNGExtension)
        .run(Cursor::new(input_bytes), writer, init_state())
        .expect("runner succeeds");

    let mut reader = HarnessInputReader::new(Cursor::new(written.bytes()));
    let mut frames = Vec::new();
    while let Some(frame) = reader.read_message().expect("read output") {
        frames.push(frame);
    }
    (state, frames)
}

fn init_state() -> tau_ext_searxng_search::SearXNGState {
    tau_ext_searxng_search::SearXNGState {
        client: tau_ext_searxng_search::SearXNGClient::new(
            "http://localhost:8080".parse().unwrap(),
            Duration::from_secs(15),
            vec![],
            vec!["local-".to_string()],
        ),
    }
}

#[derive(Clone, Default)]
struct SharedWriter {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl SharedWriter {
    fn bytes(&self) -> Vec<u8> {
        self.inner.lock().unwrap().clone()
    }
}

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        inner.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
