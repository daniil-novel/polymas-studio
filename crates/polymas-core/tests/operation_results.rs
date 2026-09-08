use polymas_core::policy::{OperationResult, ResultQuery};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[test]
fn pages_reconstruct_exact_json_without_splitting_utf8_or_repeating_bytes() {
    let value = json!({"response":{"text":"Источник 🧠\n\"\\".repeat(70)},"diagnostic":null});
    let document = OperationResult::new(value.clone()).unwrap();
    let canonical = serde_json::to_string(&value).unwrap();
    assert_eq!(document.metadata().byte_length, canonical.len());
    assert_eq!(
        document.metadata().sha256,
        format!("{:x}", Sha256::digest(canonical.as_bytes()))
    );
    for limit in [4, 7, 32, 32768] {
        let mut offset = 0;
        let mut assembled = String::new();
        loop {
            let page = document.page(ResultQuery { offset, limit }).unwrap();
            assert_eq!(page.offset, offset);
            assert_eq!(page.metadata.sha256, document.metadata().sha256);
            assert!(page.chunk.len() <= limit && !page.chunk.is_empty());
            assembled.push_str(&page.chunk);
            if let Some(next) = page.next_offset {
                assert_eq!(next, offset + page.chunk.len());
                offset = next;
            } else {
                break;
            }
        }
        assert_eq!(assembled, canonical);
        assert_eq!(serde_json::from_str::<Value>(&assembled).unwrap(), value);
    }
    let inside_utf8 = canonical.find('🧠').unwrap() + 1;
    for query in [
        ResultQuery {
            offset: inside_utf8,
            limit: 4,
        },
        ResultQuery {
            offset: canonical.len() + 1,
            limit: 4,
        },
        ResultQuery {
            offset: 0,
            limit: 0,
        },
        ResultQuery {
            offset: 0,
            limit: 32769,
        },
    ] {
        assert!(document.page(query).is_err());
    }
    let end = document
        .page(ResultQuery {
            offset: canonical.len(),
            limit: 4,
        })
        .unwrap();
    assert!(end.chunk.is_empty() && end.next_offset.is_none());
}

#[test]
fn large_results_are_not_inlined_and_wire_pages_remain_bounded_after_json_escaping() {
    let value = json!({"text":"\"\\\n".repeat(300_000)});
    let document = OperationResult::new(value).unwrap();
    assert!(document.inline().unwrap().is_none());
    let page = document.page(ResultQuery::default()).unwrap();
    let mcp = json!({"content":[{"type":"text","text":serde_json::to_string(&page).unwrap()}]});
    assert!(mcp.to_string().len() < 256 * 1024);
    assert!(page.next_offset.is_some());
    let small = OperationResult::new(json!({"marker":"exact-response"})).unwrap();
    assert_eq!(small.inline().unwrap().unwrap()["marker"], "exact-response");
    assert!(OperationResult::new(json!({"text":"x".repeat(4 * 1024 * 1024 + 8192)})).is_err());
}
