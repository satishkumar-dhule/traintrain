//! Unit tests for the SSE wire decoder, driven by frames captured verbatim
//! from `https://opencode.ai/zen/v1/chat/completions` (free model, keyless).

use super::client::Frame;
use super::client::{decode_error_body, parse_data_frame, AiEvent, Parsed, SseDecoder};

fn events_from(raw: &str) -> Vec<Result<AiEvent, crate::core::error::AppError>> {
    let mut dec = SseDecoder::new();
    dec.feed(raw.as_bytes());
    let mut out = Vec::new();
    loop {
        match dec.pop_frame().unwrap() {
            Some(Frame::Done) => {
                out.push(Ok(AiEvent::Done {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                }));
            }
            Some(Frame::Data(frame)) => match parse_data_frame(&frame) {
                Parsed::Empty => continue,
                Parsed::Error(e) => out.push(Err(e)),
                Parsed::Events(evs) => out.extend(evs.into_iter().map(Ok)),
            },
            None => break,
        }
    }
    out
}

#[test]
fn decodes_reasoning_then_content_chunks() {
    let raw = concat!(
        "data: {\"id\":\"1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"The\"}}]}\n\n",
        "data: {\"id\":\"1\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\" user has\"}}]}\n\n",
        "data: {\"id\":\"1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"id\":\"1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    let evs = events_from(raw);
    assert_eq!(
        evs,
        vec![
            Ok(AiEvent::Reasoning("The".into())),
            Ok(AiEvent::Reasoning(" user has".into())),
            Ok(AiEvent::Delta("Hello".into())),
            Ok(AiEvent::Delta(" world".into())),
            Ok(AiEvent::Done {
                prompt_tokens: 0,
                completion_tokens: 0
            }),
        ]
    );
}

#[test]
fn handles_crlf_and_split_chunk_boundaries() {
    let full = "data: {\"choices\":[{\"delta\":{\"content\":\"AB\"}}]}\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"CD\"}}]}\r\n\r\ndata: [DONE]\r\n\r\n";
    // Split at an awkward byte offset (mid-JSON of frame 2).
    let (a, b) = full.split_at(full.len() - 30);
    let mut dec = SseDecoder::new();
    dec.feed(a.as_bytes());
    let mut count = 0;
    if let Some(Frame::Data(d)) = dec.pop_frame().unwrap() {
        assert!(d.contains("AB"));
        count += 1;
    }
    dec.feed(b.as_bytes());
    while let Some(f) = dec.pop_frame().unwrap() {
        match f {
            Frame::Done => {}
            Frame::Data(d) => assert!(d.contains("CD")),
        }
        count += 1;
    }
    assert_eq!(count, 3);
}

#[test]
fn usage_chunk_becomes_terminal_with_tokens() {
    let raw = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":92,\"completion_tokens\":10}}\n\n",
        "data: [DONE]\n\n"
    );
    let mut dec = SseDecoder::new();
    dec.feed(raw.as_bytes());
    let mut got = Vec::new();
    while let Some(Frame::Data(f)) = dec.pop_frame().unwrap() {
        match parse_data_frame(&f) {
            Parsed::Events(evs) => got.extend(evs),
            Parsed::Empty => {}
            Parsed::Error(e) => panic!("unexpected error: {e}"),
        }
    }
    assert_eq!(
        got,
        vec![
            AiEvent::Delta("hi".into()),
            // At the parse layer the usage frame yields a Done event; the
            // stream layer (StreamState) promotes it to terminal instead.
            AiEvent::Done {
                prompt_tokens: 92,
                completion_tokens: 10
            },
        ]
    );
}

#[test]
fn zen_error_frame_is_typed_source_unavailable() {
    let raw = "data: {\"type\":\"error\",\"error\":{\"type\":\"FreeUsageLimitError\",\"message\":\"Rate limit exceeded.\"}}\n\n";
    let evs = events_from(raw);
    match &evs[0] {
        Err(crate::core::error::AppError::SourceUnavailable { source, reason }) => {
            assert_eq!(source, "zen");
            assert!(reason.contains("FreeUsageLimitError"), "{reason}");
            assert!(reason.contains("Rate limit exceeded."), "{reason}");
        }
        other => panic!("expected source_unavailable, got {other:?}"),
    }
}

#[test]
fn unparsable_frames_are_skipped_not_fatal() {
    let raw = concat!(
        "data: not-json-at-all\n\n",
        ": keep-alive comment\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    let evs = events_from(raw);
    assert_eq!(
        evs,
        vec![
            Ok(AiEvent::Delta("ok".into())),
            Ok(AiEvent::Done {
                prompt_tokens: 0,
                completion_tokens: 0
            })
        ]
    );
}

#[test]
fn multiline_data_field_is_joined() {
    let raw = "data: {\"choices\":[\n data still same frame\n\ndata: {\"x\":1}\n\n";
    // First block joins to invalid JSON -> skipped leniently; second parses.
    let mut dec = SseDecoder::new();
    dec.feed(raw.as_bytes());
    let mut parsed_ok = false;
    while let Some(f) = dec.pop_frame().unwrap() {
        if let Frame::Data(d) = f {
            if d.contains("\"x\":1") {
                parsed_ok = true;
            }
        }
    }
    assert!(parsed_ok);
}

#[test]
fn flush_tail_handles_missing_done() {
    let mut dec = SseDecoder::new();
    dec.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"tail\"}}]}\n\n");
    assert!(dec.pop_frame().unwrap().is_some());
    dec.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"end\"}}]}");
    assert!(dec.pop_frame().unwrap().is_none());
    match dec.flush_tail().unwrap() {
        Some(Frame::Data(f)) => {
            assert!(matches!(parse_data_frame(&f), Parsed::Events(_)));
        }
        other => panic!("expected tail data frame, got {other:?}"),
    }
}

#[test]
fn decode_error_body_understands_both_shapes() {
    let zen = r#"{"type":"error","error":{"type":"FreeUsageLimitError","message":"Rate limit exceeded. Please try again later."}}"#;
    assert_eq!(
        decode_error_body(zen).as_deref(),
        Some("FreeUsageLimitError: Rate limit exceeded. Please try again later.")
    );
    let openai = r#"{"error":{"message":"Incorrect API key"}}"#;
    assert_eq!(
        decode_error_body(openai).as_deref(),
        Some("Incorrect API key")
    );
    assert_eq!(decode_error_body("<html>oops</html>"), None);
}
