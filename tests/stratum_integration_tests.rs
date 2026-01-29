//! Integration tests for Stratum protocol
//!
//! Tests the complete stratum communication flow:
//! - Mining.subscribe
//! - Mining.authorize
//! - Mining.notify (job reception)
//! - Mining.set_difficulty
//! - Mining.submit (share submission)

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use serde_json::json;

use btcminer::stratum::StratumV1Client;

/// Helper to run a mock stratum server
fn spawn_mock_stratum_server(responses: Vec<String>) -> (String, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

            let reader = BufReader::new(stream.try_clone().unwrap());
            let mut lines = reader.lines();

            for response in responses.iter() {
                // Wait for request
                if lines.next().is_none() {
                    break;
                }

                // Send response
                let _ = writeln!(stream, "{}", response);
            }
        }
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(50));

    ("127.0.0.1".to_string(), port)
}

/// Test stratum client creation
#[test]
fn test_stratum_client_new() {
    let client = StratumV1Client::new();
    assert!(!client.is_connected());
    assert_eq!(client.get_difficulty(), 1.0);
    assert!(client.get_extranonce1().is_empty());
    assert_eq!(client.get_extranonce2_size(), 4);
}

/// Test stratum client default
#[test]
fn test_stratum_client_default() {
    let client = StratumV1Client::default();
    assert!(!client.is_connected());
}

/// Test stratum connection to mock server
#[test]
fn test_stratum_connect() {
    // Subscribe response
    let subscribe_response = json!({
        "id": 1,
        "result": [
            [["mining.set_difficulty", "subscription_id1"], ["mining.notify", "subscription_id2"]],
            "08000002",  // extranonce1
            4            // extranonce2_size
        ],
        "error": null
    }).to_string();

    let (host, port) = spawn_mock_stratum_server(vec![subscribe_response]);

    let mut client = StratumV1Client::new();
    let result = client.connect(&format!("{}:{}", host, port));

    assert!(result.is_ok(), "Should connect successfully: {:?}", result.err());
    assert!(client.is_connected());
}

/// Test stratum subscribe
#[test]
fn test_stratum_subscribe() {
    let subscribe_response = json!({
        "id": 1,
        "result": [
            [["mining.set_difficulty", "sub1"], ["mining.notify", "sub2"]],
            "deadbeef",  // extranonce1
            8            // extranonce2_size
        ],
        "error": null
    }).to_string();

    let (host, port) = spawn_mock_stratum_server(vec![subscribe_response.clone()]);

    let mut client = StratumV1Client::new();
    client.connect(&format!("{}:{}", host, port)).unwrap();
    let result = client.subscribe();

    assert!(result.is_ok());
    assert_eq!(client.get_extranonce1(), hex::decode("deadbeef").unwrap());
    assert_eq!(client.get_extranonce2_size(), 8);
}

/// Test stratum authorize success
#[test]
fn test_stratum_authorize_success() {
    let subscribe_response = json!({
        "id": 1,
        "result": [[[], []], "08000002", 4],
        "error": null
    }).to_string();

    let authorize_response = json!({
        "id": 2,
        "result": true,
        "error": null
    }).to_string();

    let (host, port) = spawn_mock_stratum_server(vec![subscribe_response, authorize_response]);

    let mut client = StratumV1Client::new();
    client.connect(&format!("{}:{}", host, port)).unwrap();
    client.subscribe().unwrap();

    let result = client.authorize("bc1qtest.worker1", "x");
    assert!(result.is_ok());
}

/// Test stratum authorize failure
#[test]
fn test_stratum_authorize_failure() {
    let subscribe_response = json!({
        "id": 1,
        "result": [[[], []], "08000002", 4],
        "error": null
    }).to_string();

    let authorize_response = json!({
        "id": 2,
        "result": false,
        "error": "Invalid worker"
    }).to_string();

    let (host, port) = spawn_mock_stratum_server(vec![subscribe_response, authorize_response]);

    let mut client = StratumV1Client::new();
    client.connect(&format!("{}:{}", host, port)).unwrap();
    client.subscribe().unwrap();

    let result = client.authorize("invalid", "x");
    assert!(result.is_err());
}

/// Test stratum job notification parsing
#[test]
fn test_stratum_job_notification_parsing() {
    use btcminer::stratum::v1::Notification;

    let mut client = StratumV1Client::new();

    // Simulate parsing a mining.notify message
    let notify_msg = json!({
        "id": null,
        "method": "mining.notify",
        "params": [
            "bf",                                                              // job_id
            "4d16b6f85af6e2198f44ae2a6de67f78487ae5611b77c6c0440b921e00000000", // prevhash
            "01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff20020862062f503253482f04b8864e5008",  // coinbase1
            "072f736c7573682f000000000100f2052a010000001976a914d23fcdf86f7e756a64a7a9688ef9903327048ed988ac00000000", // coinbase2
            [],                                                                // merkle_branches
            "00000002",                                                        // version
            "1c2ac4af",                                                        // nbits
            "504e86b9",                                                        // ntime
            false                                                              // clean_jobs
        ]
    });

    // Parse the notification
    let result = client.parse_notification(&notify_msg);
    assert!(result.is_ok());

    let notification = result.unwrap();
    assert!(notification.is_some());

    if let Some(Notification::Job(job)) = notification {
        assert_eq!(job.id, "bf");
        assert_eq!(job.version, 0x00000002);
        assert_eq!(job.nbits, 0x1c2ac4af);
        assert_eq!(job.ntime, 0x504e86b9);
        assert!(!job.clean_jobs);
    } else {
        panic!("Expected Job notification");
    }
}

/// Test difficulty notification parsing
#[test]
fn test_stratum_difficulty_notification() {
    let mut client = StratumV1Client::new();

    let diff_msg = json!({
        "id": null,
        "method": "mining.set_difficulty",
        "params": [16.0]
    });

    let result = client.parse_notification(&diff_msg);
    assert!(result.is_ok());

    let notification = result.unwrap();
    assert!(notification.is_some());

    if let Some(btcminer::stratum::v1::Notification::Difficulty(diff)) = notification {
        assert!((diff - 16.0).abs() < 0.001);
        assert!((client.get_difficulty() - 16.0).abs() < 0.001);
    } else {
        panic!("Expected Difficulty notification");
    }
}

/// Test share submission format
#[test]
fn test_stratum_share_submission_format() {
    let subscribe_response = json!({
        "id": 1,
        "result": [[[], []], "08000002", 4],
        "error": null
    }).to_string();

    let authorize_response = json!({
        "id": 2,
        "result": true,
        "error": null
    }).to_string();

    let submit_response = json!({
        "id": 3,
        "result": true,
        "error": null
    }).to_string();

    let (host, port) = spawn_mock_stratum_server(vec![
        subscribe_response,
        authorize_response,
        submit_response
    ]);

    let mut client = StratumV1Client::new();
    client.connect(&format!("{}:{}", host, port)).unwrap();
    client.subscribe().unwrap();
    client.authorize("worker", "x").unwrap();

    // Submit a share
    let result = client.submit(
        "worker",
        "bf",           // job_id
        "00000001",     // extranonce2
        "504e86ed",     // ntime
        "b2957c02"      // nonce
    );

    assert!(result.is_ok());
    assert!(result.unwrap()); // Share accepted
}

/// Test share rejection
#[test]
fn test_stratum_share_rejection() {
    let subscribe_response = json!({
        "id": 1,
        "result": [[[], []], "08000002", 4],
        "error": null
    }).to_string();

    let authorize_response = json!({
        "id": 2,
        "result": true,
        "error": null
    }).to_string();

    let submit_response = json!({
        "id": 3,
        "result": false,  // Rejected
        "error": null
    }).to_string();

    let (host, port) = spawn_mock_stratum_server(vec![
        subscribe_response,
        authorize_response,
        submit_response
    ]);

    let mut client = StratumV1Client::new();
    client.connect(&format!("{}:{}", host, port)).unwrap();
    client.subscribe().unwrap();
    client.authorize("worker", "x").unwrap();

    let result = client.submit("worker", "bf", "00000001", "504e86ed", "badnonce");
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Share rejected
}

/// Test unknown notification is ignored
#[test]
fn test_stratum_unknown_notification() {
    let mut client = StratumV1Client::new();

    let unknown_msg = json!({
        "id": null,
        "method": "mining.unknown_method",
        "params": []
    });

    let result = client.parse_notification(&unknown_msg);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none()); // Unknown methods return None
}

/// Test message without method field
#[test]
fn test_stratum_message_no_method() {
    let mut client = StratumV1Client::new();

    // Response message (not a notification)
    let response_msg = json!({
        "id": 1,
        "result": true,
        "error": null
    });

    let result = client.parse_notification(&response_msg);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

/// Test extranonce parsing
#[test]
fn test_extranonce_hex_parsing() {
    // Various extranonce1 values from real pools
    let test_cases = vec![
        ("08000002", vec![0x08, 0x00, 0x00, 0x02]),
        ("deadbeef", vec![0xde, 0xad, 0xbe, 0xef]),
        ("12345678", vec![0x12, 0x34, 0x56, 0x78]),
        ("00000000", vec![0x00, 0x00, 0x00, 0x00]),
        ("ffffffff", vec![0xff, 0xff, 0xff, 0xff]),
    ];

    for (hex_str, expected) in test_cases {
        let decoded = hex::decode(hex_str).unwrap();
        assert_eq!(decoded, expected);
    }
}

/// Test nonce encoding for submission
#[test]
fn test_nonce_encoding() {
    // Nonces are submitted as big-endian hex
    let nonce: u32 = 0xdeadbeef;

    // For stratum v1, nonce is sent as little-endian hex
    let le_bytes = nonce.to_le_bytes();
    let hex_le = hex::encode(le_bytes);
    assert_eq!(hex_le, "efbeadde");

    // Some pools expect big-endian
    let be_bytes = nonce.to_be_bytes();
    let hex_be = hex::encode(be_bytes);
    assert_eq!(hex_be, "deadbeef");
}

/// Test ntime encoding for submission
#[test]
fn test_ntime_encoding() {
    let ntime: u32 = 0x504e86ed;

    // ntime is sent as big-endian hex
    let hex = format!("{:08x}", ntime);
    assert_eq!(hex, "504e86ed");

    // Verify round-trip
    let parsed = u32::from_str_radix(&hex, 16).unwrap();
    assert_eq!(parsed, ntime);
}

/// Test disconnect handling
#[test]
fn test_stratum_disconnect() {
    let subscribe_response = json!({
        "id": 1,
        "result": [[[], []], "08000002", 4],
        "error": null
    }).to_string();

    let (host, port) = spawn_mock_stratum_server(vec![subscribe_response]);

    let mut client = StratumV1Client::new();
    client.connect(&format!("{}:{}", host, port)).unwrap();
    assert!(client.is_connected());

    client.disconnect();
    assert!(!client.is_connected());
}

/// Test connection to invalid address
#[test]
fn test_stratum_invalid_connection() {
    let mut client = StratumV1Client::new();

    // Try to connect to a non-existent server
    let result = client.connect("127.0.0.1:59999");
    assert!(result.is_err());
    assert!(!client.is_connected());
}

/// Test job with merkle branches
#[test]
fn test_stratum_job_with_merkle_branches() {
    let mut client = StratumV1Client::new();

    let notify_msg = json!({
        "method": "mining.notify",
        "params": [
            "job123",
            "0".repeat(64),
            "",
            "",
            [
                "a".repeat(64),
                "b".repeat(64),
                "c".repeat(64)
            ],
            "20000000",
            "1d00ffff",
            "60000000",
            true
        ]
    });

    let result = client.parse_notification(&notify_msg);
    assert!(result.is_ok());

    if let Some(btcminer::stratum::v1::Notification::Job(job)) = result.unwrap() {
        assert_eq!(job.id, "job123");
        assert_eq!(job.merkle_branches.len(), 3);
        assert!(job.clean_jobs);
    } else {
        panic!("Expected Job notification");
    }
}
