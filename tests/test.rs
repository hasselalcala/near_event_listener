use near_event_listener::{ListenerError, NearEventListener};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_success() {
        let listener = NearEventListener::builder("http://rpc.testnet.near.org")
            .account_id("test.near")
            .method_name("nft_mint")
            .last_processed_block(1234)
            .build();

        assert!(listener.is_ok());
        let listener = listener.unwrap();
        assert_eq!(listener.account_id, "test.near");
        assert_eq!(listener.method_name, "nft_mint");
        assert_eq!(listener.last_processed_block, 1234);
    }

    #[test]
    fn test_builder_missing_account_id() {
        let listener = NearEventListener::builder("http://rpc.testnet.near.org")
            .method_name("nft_mint")
            .build();

        assert!(matches!(
            listener.unwrap_err(),
            ListenerError::MissingField(field) if field == "account_id"
        ));
    }

    #[test]
    fn test_builder_missing_method_name() {
        let listener = NearEventListener::builder("http://rpc.testnet.near.org")
            .account_id("test.near")
            .build();

        assert!(matches!(
            listener.unwrap_err(),
            ListenerError::MissingField(field) if field == "method_name"
        ));
    }

    // Tests for log processing
    #[test]
    fn test_process_log_success() {
        let log = r#"EVENT_JSON:{"standard":"nep171","version":"1.0.0","event":"nft_mint","data":{"token_ids":["1","2"]}}"#;
        let result = NearEventListener::process_log(log);

        assert!(result.is_ok());
        let event_log = result.unwrap();
        assert_eq!(event_log.standard, "nep171");
        assert_eq!(event_log.version, "1.0.0");
        assert_eq!(event_log.event, "nft_mint");
    }

    #[test]
    fn test_process_log_invalid_format() {
        let log = "Invalid log format";
        let result = NearEventListener::process_log(log);

        assert!(matches!(
            result.unwrap_err(),
            ListenerError::InvalidEventFormat(_)
        ));
    }

    #[test]
    fn test_process_log_invalid_json() {
        let log = r#"EVENT_JSON:{"standard":"nep171","version":1.0.0,invalid_json}"#;
        let result = NearEventListener::process_log(log);

        assert!(matches!(result.unwrap_err(), ListenerError::JsonError(_)));
    }
}
