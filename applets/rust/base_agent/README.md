## To deploy
```
deploy -e --widl-file /home/ubuntu/weilliptic/mcp-vault/rust/base_agent/base_agent.widl --file-path /home/ubuntu/weilliptic/mcp-vault/rust/base_agent/target/wasm32-unknown-unknown/release/base_agent.wasm -i '{"description":"aurora agent", "mcp_contract_address":"aaaaaaqq7gdsyamjzj3ok4hv3fg6koii7tmksr3oioholpyxalyig44sr4"}'
```

## To run a task
```execute -n aaaaaaqtlagfzlsu5lbxetywnl33z3pyah4jep3htg6ewb6q55tul2t5v4 -m run_task -i '{"task_prompt":"get 5 album titles"}'```

or,

Switch to the `base_client` project, modify the `private_key` and `contract_id` variables and run
```
cargo run
```