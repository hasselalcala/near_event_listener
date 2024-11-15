test:
    echo "Running tests"
    cargo test --test test_sandbox_integration -- --nocapture

test_2:
    echo "Running tests"
    cargo test --test test_testnet_integration -- --nocapture
